use std::io::Write;

const TEXTURE_DIMS: (usize, usize) = (512, 512);

const VERTEX_SOURCE: &'static str = "
#version 450
#extension GL_EXT_debug_printf : require
#extension GL_EXT_spirv_intrinsics : require

const vec2 positions[3] = vec2[3](
    vec2( 0.0, -0.8),
    vec2( 1.0, -1.0),
    vec2(-1.0, -1.0)
);

const vec2 uvs[3] = vec2[3](
    vec2(0.5, 0.0),
    vec2(1.0, 1.0),
    vec2(0.0, 1.0)
);

layout(location=0) out vec2 uv;

void main() {
    gl_Position = vec4(positions[gl_VertexIndex], 0.0, 1.0);
    uv = uvs[gl_VertexIndex];
}";

const FRAGMENT_SOURCE: &'static str = "
#version 450
#extension GL_EXT_debug_printf : require
#extension GL_EXT_spirv_intrinsics : require

layout(location=0) in vec2 uv;
layout(location=0) out vec4 color;

layout(binding=0) uniform texture2D textureImage;
layout(binding=1) uniform sampler textureSampler;

void main() {
    vec2 queryLod = textureQueryLOD(sampler2D(textureImage, textureSampler), uv);
    color = vec4(queryLod.x, queryLod.y, 2.5, 1.0);
}";

fn create_mip_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
    let mip_level_count = ((width.max(height) as f32).log2().floor() as u32) + 1;

    let texture_descriptor = wgpu::TextureDescriptor {
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        label: None,
        view_formats: &[wgpu::TextureFormat::Rgba8Unorm],
    };

    device.create_texture(&texture_descriptor)
}

pub fn output_image_native(image_data: Vec<u8>, texture_dims: (usize, usize), path: String) {
    // Open a file in write mode
    let mut file = std::fs::File::create(&path).expect("Failed to create file");

    // Write the raw image data to the file
    file.write_all(&image_data)
        .expect("Failed to write to file");
}

async fn run(_path: Option<String>) {
    let mut texture_data = Vec::<u8>::with_capacity(TEXTURE_DIMS.0 * TEXTURE_DIMS.1 * 16);
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::VULKAN,
        flags: wgpu::InstanceFlags::debugging().with_env(),
        dx12_shader_compiler: wgpu::util::dx12_shader_compiler_from_env().unwrap_or_default(),
        gles_minor_version: wgpu::util::gles_minor_version_from_env().unwrap_or_default(),
    });
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions::default())
        .await
        .unwrap();
    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::SPIRV_SHADER_PASSTHROUGH,
                required_limits: wgpu::Limits::downlevel_defaults(),
            },
            None,
        )
        .await
        .unwrap();
    let compiler = shaderc::Compiler::new().unwrap();
    let vert_spv = compiler
        .compile_into_spirv(
            VERTEX_SOURCE.trim(),
            shaderc::ShaderKind::Vertex,
            "vert.glsl",
            "main",
            None,
        )
        .unwrap();
    let frag_spv = compiler
        .compile_into_spirv(
            FRAGMENT_SOURCE.trim(),
            shaderc::ShaderKind::Fragment,
            "frag.glsl",
            "main",
            None,
        )
        .unwrap();
    let vert_descriptor = wgpu::ShaderModuleDescriptorSpirV {
        label: None,
        source: std::borrow::Cow::Borrowed(vert_spv.as_binary()),
    };
    let vert_shader = unsafe { device.create_shader_module_spirv(&vert_descriptor) };
    let frag_descriptor = wgpu::ShaderModuleDescriptorSpirV {
        label: None,
        source: std::borrow::Cow::Borrowed(frag_spv.as_binary()),
    };
    let frag_shader = unsafe { device.create_shader_module_spirv(&frag_descriptor) };
    let render_target = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: TEXTURE_DIMS.0 as u32,
            height: TEXTURE_DIMS.1 as u32,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[wgpu::TextureFormat::Rgba32Float],
    });
    let output_staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: texture_data.capacity() as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: None,
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &vert_shader,
            entry_point: "main",
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &frag_shader,
            entry_point: "main",
            targets: &[Some(wgpu::TextureFormat::Rgba32Float.into())],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    });
    let mip_texture = create_mip_texture(&device, TEXTURE_DIMS.0 as u32, TEXTURE_DIMS.1 as u32);
    let mip_texture_view = mip_texture.create_view(&wgpu::TextureViewDescriptor::default());
    let bind_group_layout = pipeline.get_bind_group_layout(0);
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: None,
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Linear,
        anisotropy_clamp: 1,
        lod_min_clamp: 0.0,
        lod_max_clamp: std::f32::MAX,
        ..Default::default()
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&mip_texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
        ],
        label: None,
    });

    let texture_view = render_target.create_view(&wgpu::TextureViewDescriptor::default());
    let mut command_encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    {
        let mut render_pass = command_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &texture_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });
        render_pass.set_pipeline(&pipeline);
        render_pass.set_bind_group(0, &bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }
    command_encoder.copy_texture_to_buffer(
        wgpu::ImageCopyTexture {
            texture: &render_target,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::ImageCopyBuffer {
            buffer: &output_staging_buffer,
            layout: wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some((TEXTURE_DIMS.0 * 16) as u32),
                rows_per_image: Some(TEXTURE_DIMS.1 as u32),
            },
        },
        wgpu::Extent3d {
            width: TEXTURE_DIMS.0 as u32,
            height: TEXTURE_DIMS.1 as u32,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(Some(command_encoder.finish()));

    let buffer_slice = output_staging_buffer.slice(..);
    let (sender, receiver) = flume::bounded(1);
    buffer_slice.map_async(wgpu::MapMode::Read, move |r| sender.send(r).unwrap());
    device.poll(wgpu::Maintain::wait()).panic_on_timeout();
    receiver.recv_async().await.unwrap().unwrap();
    let view = buffer_slice.get_mapped_range();
    texture_data.extend_from_slice(&view[..]);

    output_image_native(texture_data.to_vec(), TEXTURE_DIMS, _path.unwrap());
}

pub fn main() {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "output.vulkan.bin".to_string());
    pollster::block_on(run(Some(path)));
}
