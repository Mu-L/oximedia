// Convolution filter shaders for blur, sharpen, and custom kernels

@group(0) @binding(0) var<storage, read> input_image: array<u32>;
@group(0) @binding(1) var<storage, read_write> output_image: array<u32>;
@group(0) @binding(2) var<uniform> params: FilterParams;
@group(0) @binding(3) var<storage, read> kernel: array<f32>;

struct FilterParams {
    width: u32,
    height: u32,
    stride: u32,
    kernel_size: u32, // Must be odd (3, 5, 7, etc.)
    normalize: u32,   // 1 to normalize kernel, 0 otherwise
    filter_type: u32, // 0=custom, 1=gaussian, 2=sharpen, 3=edge, 4=emboss
    padding: u32,
    sigma: f32,       // For gaussian blur
}

fn unpack_rgba(packed: u32) -> vec4<f32> {
    let r = f32((packed >> 24u) & 0xFFu) / 255.0;
    let g = f32((packed >> 16u) & 0xFFu) / 255.0;
    let b = f32((packed >> 8u) & 0xFFu) / 255.0;
    let a = f32(packed & 0xFFu) / 255.0;
    return vec4<f32>(r, g, b, a);
}

fn pack_rgba(color: vec4<f32>) -> u32 {
    let r = u32(clamp(color.r * 255.0, 0.0, 255.0));
    let g = u32(clamp(color.g * 255.0, 0.0, 255.0));
    let b = u32(clamp(color.b * 255.0, 0.0, 255.0));
    let a = u32(clamp(color.a * 255.0, 0.0, 255.0));
    return (r << 24u) | (g << 16u) | (b << 8u) | a;
}

fn sample_image(x: i32, y: i32) -> vec4<f32> {
    let cx = clamp(x, 0, i32(params.width) - 1);
    let cy = clamp(y, 0, i32(params.height) - 1);
    let idx = u32(cy) * params.stride + u32(cx);
    return unpack_rgba(input_image[idx]);
}

// Gaussian weight calculation
fn gaussian_weight(x: i32, y: i32, sigma: f32) -> f32 {
    let fx = f32(x);
    let fy = f32(y);
    let sigma2 = sigma * sigma;
    return exp(-(fx * fx + fy * fy) / (2.0 * sigma2)) / (6.28318530718 * sigma2);
}

@compute @workgroup_size(16, 16, 1)
fn convolve_main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let x = i32(global_id.x);
    let y = i32(global_id.y);

    if (u32(x) >= params.width || u32(y) >= params.height) {
        return;
    }

    let radius = i32(params.kernel_size) / 2;
    var sum = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    var weight_sum = 0.0;

    for (var ky = -radius; ky <= radius; ky = ky + 1) {
        for (var kx = -radius; kx <= radius; kx = kx + 1) {
            let px = x + kx;
            let py = y + ky;

            let sample = sample_image(px, py);
            var weight = 0.0;

            if (params.filter_type == 1u) {
                // Gaussian blur
                weight = gaussian_weight(kx, ky, params.sigma);
            } else {
                // Custom kernel
                let kernel_x = kx + radius;
                let kernel_y = ky + radius;
                let kernel_idx = u32(kernel_y) * params.kernel_size + u32(kernel_x);
                weight = kernel[kernel_idx];
            }

            sum += sample * weight;
            weight_sum += weight;
        }
    }

    var result: vec4<f32>;
    if (params.normalize == 1u && weight_sum > 0.0) {
        result = sum / weight_sum;
    } else {
        result = sum;
    }

    // Clamp to [0, 1] range
    result = clamp(result, vec4<f32>(0.0), vec4<f32>(1.0));

    let idx = u32(y) * params.stride + u32(x);
    output_image[idx] = pack_rgba(result);
}

// Separable filter for efficient gaussian blur (horizontal pass)
@compute @workgroup_size(256, 1, 1)
fn separable_horizontal(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let idx = global_id.x;
    let y = idx / params.width;
    let x = idx % params.width;

    if (x >= params.width || y >= params.height) {
        return;
    }

    let radius = i32(params.kernel_size) / 2;
    var sum = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    var weight_sum = 0.0;

    for (var kx = -radius; kx <= radius; kx = kx + 1) {
        let px = i32(x) + kx;
        let sample = sample_image(px, i32(y));

        let weight = gaussian_weight(kx, 0, params.sigma);
        sum += sample * weight;
        weight_sum += weight;
    }

    let result = sum / weight_sum;
    let out_idx = y * params.stride + x;
    output_image[out_idx] = pack_rgba(result);
}

// Separable filter for efficient gaussian blur (vertical pass)
@compute @workgroup_size(256, 1, 1)
fn separable_vertical(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let idx = global_id.x;
    let y = idx / params.width;
    let x = idx % params.width;

    if (x >= params.width || y >= params.height) {
        return;
    }

    let radius = i32(params.kernel_size) / 2;
    var sum = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    var weight_sum = 0.0;

    for (var ky = -radius; ky <= radius; ky = ky + 1) {
        let py = i32(y) + ky;
        let sample = sample_image(i32(x), py);

        let weight = gaussian_weight(0, ky, params.sigma);
        sum += sample * weight;
        weight_sum += weight;
    }

    let result = sum / weight_sum;
    let out_idx = y * params.stride + x;
    output_image[out_idx] = pack_rgba(result);
}

// Edge detection using Sobel operator
@compute @workgroup_size(16, 16, 1)
fn edge_detect(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let x = i32(global_id.x);
    let y = i32(global_id.y);

    if (u32(x) >= params.width || u32(y) >= params.height) {
        return;
    }

    // Sobel kernels
    // Gx: [-1  0  1]    Gy: [-1 -2 -1]
    //     [-2  0  2]         [ 0  0  0]
    //     [-1  0  1]         [ 1  2  1]

    var gx = vec3<f32>(0.0);
    var gy = vec3<f32>(0.0);

    for (var dy = -1; dy <= 1; dy = dy + 1) {
        for (var dx = -1; dx <= 1; dx = dx + 1) {
            let sample = sample_image(x + dx, y + dy).rgb;

            // Horizontal gradient
            let kx = f32(dx);
            let ky_x = select(1.0, 2.0, dy == 0);
            gx += sample * kx * ky_x;

            // Vertical gradient
            let ky = f32(dy);
            let kx_y = select(1.0, 2.0, dx == 0);
            gy += sample * ky * kx_y;
        }
    }

    let magnitude = sqrt(gx * gx + gy * gy);
    let edge_strength = length(magnitude) / 1.732; // Normalize by sqrt(3)

    let result = vec4<f32>(edge_strength, edge_strength, edge_strength, 1.0);
    let idx = u32(y) * params.stride + u32(x);
    output_image[idx] = pack_rgba(result);
}

// Unsharp mask for sharpening
@compute @workgroup_size(16, 16, 1)
fn unsharp_mask(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let x = i32(global_id.x);
    let y = i32(global_id.y);

    if (u32(x) >= params.width || u32(y) >= params.height) {
        return;
    }

    let original = sample_image(x, y);

    // Apply gaussian blur
    let radius = i32(params.kernel_size) / 2;
    var blurred = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    var weight_sum = 0.0;

    for (var ky = -radius; ky <= radius; ky = ky + 1) {
        for (var kx = -radius; kx <= radius; kx = kx + 1) {
            let sample = sample_image(x + kx, y + ky);
            let weight = gaussian_weight(kx, ky, params.sigma);
            blurred += sample * weight;
            weight_sum += weight;
        }
    }

    blurred /= weight_sum;

    // Unsharp mask: original + amount * (original - blurred)
    let amount = 1.5; // Sharpening strength
    let sharpened = original + amount * (original - blurred);
    let result = clamp(sharpened, vec4<f32>(0.0), vec4<f32>(1.0));

    let idx = u32(y) * params.stride + u32(x);
    output_image[idx] = pack_rgba(result);
}
