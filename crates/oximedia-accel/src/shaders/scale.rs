//! Image scaling compute shaders.

/// Bilinear scaling compute shader.
#[allow(missing_docs)]
pub mod bilinear {
    vulkano_shaders::shader! {
        ty: "compute",
        src: r"
            #version 450

            layout(local_size_x = 16, local_size_y = 16) in;

            layout(set = 0, binding = 0) buffer InputBuffer {
                uint data[];
            } input_buf;

            layout(set = 0, binding = 1) buffer OutputBuffer {
                uint data[];
            } output_buf;

            layout(push_constant) uniform PushConstants {
                uint src_width;
                uint src_height;
                uint dst_width;
                uint dst_height;
                uint channels;
            } pc;

            vec4 sample_pixel(uint x, uint y) {
                uint idx = (y * pc.src_width + x) * pc.channels;
                if (pc.channels == 3) {
                    uint packed = input_buf.data[idx / 4];
                    uint shift = (idx % 4) * 8;
                    float r = float((packed >> shift) & 0xFF) / 255.0;
                    float g = float((packed >> ((shift + 8) % 32)) & 0xFF) / 255.0;
                    float b = float((packed >> ((shift + 16) % 32)) & 0xFF) / 255.0;
                    return vec4(r, g, b, 1.0);
                } else {
                    // Assume 4 channels
                    uint packed = input_buf.data[idx / 4];
                    float r = float((packed >> 0) & 0xFF) / 255.0;
                    float g = float((packed >> 8) & 0xFF) / 255.0;
                    float b = float((packed >> 16) & 0xFF) / 255.0;
                    float a = float((packed >> 24) & 0xFF) / 255.0;
                    return vec4(r, g, b, a);
                }
            }

            void main() {
                uint gid_x = gl_GlobalInvocationID.x;
                uint gid_y = gl_GlobalInvocationID.y;

                if (gid_x >= pc.dst_width || gid_y >= pc.dst_height) {
                    return;
                }

                float x_ratio = float(pc.src_width - 1) / float(pc.dst_width);
                float y_ratio = float(pc.src_height - 1) / float(pc.dst_height);

                float src_x = float(gid_x) * x_ratio;
                float src_y = float(gid_y) * y_ratio;

                uint x1 = uint(floor(src_x));
                uint y1 = uint(floor(src_y));
                uint x2 = min(x1 + 1, pc.src_width - 1);
                uint y2 = min(y1 + 1, pc.src_height - 1);

                float x_frac = src_x - float(x1);
                float y_frac = src_y - float(y1);

                vec4 p11 = sample_pixel(x1, y1);
                vec4 p12 = sample_pixel(x1, y2);
                vec4 p21 = sample_pixel(x2, y1);
                vec4 p22 = sample_pixel(x2, y2);

                vec4 p1 = mix(p11, p21, x_frac);
                vec4 p2 = mix(p12, p22, x_frac);
                vec4 result = mix(p1, p2, y_frac);

                uint out_idx = (gid_y * pc.dst_width + gid_x) * pc.channels;
                if (pc.channels == 3) {
                    uint r = uint(clamp(result.r * 255.0, 0.0, 255.0));
                    uint g = uint(clamp(result.g * 255.0, 0.0, 255.0));
                    uint b = uint(clamp(result.b * 255.0, 0.0, 255.0));
                    uint packed = r | (g << 8) | (b << 16);
                    output_buf.data[out_idx / 4] = packed;
                } else {
                    uint r = uint(clamp(result.r * 255.0, 0.0, 255.0));
                    uint g = uint(clamp(result.g * 255.0, 0.0, 255.0));
                    uint b = uint(clamp(result.b * 255.0, 0.0, 255.0));
                    uint a = uint(clamp(result.a * 255.0, 0.0, 255.0));
                    uint packed = r | (g << 8) | (b << 16) | (a << 24);
                    output_buf.data[out_idx / 4] = packed;
                }
            }
        "
    }
}

/// Nearest neighbor scaling compute shader.
#[allow(missing_docs)]
pub mod nearest {
    vulkano_shaders::shader! {
        ty: "compute",
        src: r"
            #version 450

            layout(local_size_x = 16, local_size_y = 16) in;

            layout(set = 0, binding = 0) buffer InputBuffer {
                uint data[];
            } input_buf;

            layout(set = 0, binding = 1) buffer OutputBuffer {
                uint data[];
            } output_buf;

            layout(push_constant) uniform PushConstants {
                uint src_width;
                uint src_height;
                uint dst_width;
                uint dst_height;
                uint channels;
            } pc;

            void main() {
                uint gid_x = gl_GlobalInvocationID.x;
                uint gid_y = gl_GlobalInvocationID.y;

                if (gid_x >= pc.dst_width || gid_y >= pc.dst_height) {
                    return;
                }

                uint src_x = (gid_x * pc.src_width) / pc.dst_width;
                uint src_y = (gid_y * pc.src_height) / pc.dst_height;

                uint src_idx = (src_y * pc.src_width + src_x) * pc.channels;
                uint dst_idx = (gid_y * pc.dst_width + gid_x) * pc.channels;

                // Copy pixel data
                for (uint i = 0; i < pc.channels; i++) {
                    uint src_offset = (src_idx + i) / 4;
                    uint dst_offset = (dst_idx + i) / 4;
                    output_buf.data[dst_offset] = input_buf.data[src_offset];
                }
            }
        "
    }
}
