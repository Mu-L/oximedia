//! Color conversion compute shaders.

/// RGB to `YUV420p` conversion compute shader.
#[allow(missing_docs)]
pub mod rgb_to_yuv420p {
    vulkano_shaders::shader! {
        ty: "compute",
        src: r"
            #version 450

            layout(local_size_x = 16, local_size_y = 16) in;

            layout(set = 0, binding = 0) buffer InputBuffer {
                uint data[];
            } input_buf;

            layout(set = 0, binding = 1) buffer OutputYBuffer {
                uint data[];
            } output_y;

            layout(set = 0, binding = 2) buffer OutputUBuffer {
                uint data[];
            } output_u;

            layout(set = 0, binding = 3) buffer OutputVBuffer {
                uint data[];
            } output_v;

            layout(push_constant) uniform PushConstants {
                uint width;
                uint height;
                uint input_channels;
            } pc;

            vec3 rgb_to_yuv(vec3 rgb) {
                float y = 0.299 * rgb.r + 0.587 * rgb.g + 0.114 * rgb.b;
                float u = -0.169 * rgb.r - 0.331 * rgb.g + 0.500 * rgb.b + 0.5;
                float v = 0.500 * rgb.r - 0.419 * rgb.g - 0.081 * rgb.b + 0.5;
                return vec3(y, u, v);
            }

            void main() {
                uint gid_x = gl_GlobalInvocationID.x;
                uint gid_y = gl_GlobalInvocationID.y;

                if (gid_x >= pc.width || gid_y >= pc.height) {
                    return;
                }

                // Read RGB pixel
                uint idx = (gid_y * pc.width + gid_x) * pc.input_channels;
                uint packed = input_buf.data[idx / 4];
                uint shift = (idx % 4) * 8;

                float r = float((packed >> shift) & 0xFF) / 255.0;
                float g = float((packed >> ((shift + 8) % 32)) & 0xFF) / 255.0;
                float b = float((packed >> ((shift + 16) % 32)) & 0xFF) / 255.0;

                // Convert to YUV
                vec3 yuv = rgb_to_yuv(vec3(r, g, b));

                // Write Y plane
                uint y_idx = gid_y * pc.width + gid_x;
                output_y.data[y_idx / 4] = uint(clamp(yuv.x * 255.0, 0.0, 255.0));

                // Write U and V planes (subsampled 2x2)
                if ((gid_x % 2) == 0 && (gid_y % 2) == 0) {
                    uint uv_idx = (gid_y / 2) * (pc.width / 2) + (gid_x / 2);
                    output_u.data[uv_idx / 4] = uint(clamp(yuv.y * 255.0, 0.0, 255.0));
                    output_v.data[uv_idx / 4] = uint(clamp(yuv.z * 255.0, 0.0, 255.0));
                }
            }
        "
    }
}

/// `YUV420p` to RGB conversion compute shader.
#[allow(missing_docs)]
pub mod yuv420p_to_rgb {
    vulkano_shaders::shader! {
        ty: "compute",
        src: r"
            #version 450

            layout(local_size_x = 16, local_size_y = 16) in;

            layout(set = 0, binding = 0) buffer InputYBuffer {
                uint data[];
            } input_y;

            layout(set = 0, binding = 1) buffer InputUBuffer {
                uint data[];
            } input_u;

            layout(set = 0, binding = 2) buffer InputVBuffer {
                uint data[];
            } input_v;

            layout(set = 0, binding = 3) buffer OutputBuffer {
                uint data[];
            } output_buf;

            layout(push_constant) uniform PushConstants {
                uint width;
                uint height;
                uint output_channels;
            } pc;

            vec3 yuv_to_rgb(vec3 yuv) {
                float y = yuv.x;
                float u = yuv.y - 0.5;
                float v = yuv.z - 0.5;

                float r = y + 1.402 * v;
                float g = y - 0.344 * u - 0.714 * v;
                float b = y + 1.772 * u;

                return clamp(vec3(r, g, b), 0.0, 1.0);
            }

            void main() {
                uint gid_x = gl_GlobalInvocationID.x;
                uint gid_y = gl_GlobalInvocationID.y;

                if (gid_x >= pc.width || gid_y >= pc.height) {
                    return;
                }

                // Read YUV values
                uint y_idx = gid_y * pc.width + gid_x;
                uint uv_idx = (gid_y / 2) * (pc.width / 2) + (gid_x / 2);

                float y = float(input_y.data[y_idx / 4] & 0xFF) / 255.0;
                float u = float(input_u.data[uv_idx / 4] & 0xFF) / 255.0;
                float v = float(input_v.data[uv_idx / 4] & 0xFF) / 255.0;

                // Convert to RGB
                vec3 rgb = yuv_to_rgb(vec3(y, u, v));

                // Write RGB pixel
                uint out_idx = (gid_y * pc.width + gid_x) * pc.output_channels;
                uint r = uint(rgb.r * 255.0);
                uint g = uint(rgb.g * 255.0);
                uint b = uint(rgb.b * 255.0);

                if (pc.output_channels == 3) {
                    uint packed = r | (g << 8) | (b << 16);
                    output_buf.data[out_idx / 4] = packed;
                } else {
                    uint packed = r | (g << 8) | (b << 16) | (255 << 24);
                    output_buf.data[out_idx / 4] = packed;
                }
            }
        "
    }
}
