//! The one owner of GPU state, and the only module that uses wgpu.

use glam::{Mat3, Mat4};
use serde::Serialize;
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, Sender};

use wgpu::util::DeviceExt;

use crate::frame::Frame;
use crate::mesh::{self, DrawList, Lighting, Mesh, Point, StagedGlow, StagedObject};
use crate::scene::{Background, Camera, SceneConfig};

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24PlusStencil8;
const SAMPLES: u32 = 4;

pub struct Gpu {
    pub identity: GpuIdentity,
    device: wgpu::Device,
    queue: wgpu::Queue,
    uniforms_layout: wgpu::BindGroupLayout,
    color_format: wgpu::TextureFormat,
    /// Indexed by `Pass`, then by whether a mesh is an overlay.
    pipelines: [[wgpu::RenderPipeline; 2]; 4],
    /// Kept from one frame to the next, since each takes milliseconds to make at 3840 by 2160.
    attachments: Mutex<Option<Attachments>>,
    /// Readback buffers a `Readback` has finished with, sent back by `Readback::frame` from whichever
    /// thread it ran on.
    unused_readbacks: Mutex<Receiver<wgpu::Buffer>>,
    readback_sender: Sender<wgpu::Buffer>,
}

/// How one mesh is drawn.
#[derive(Clone, Copy)]
enum Pass {
    /// Color and depth, for a mesh that hides what is behind it.
    Opaque = 0,
    /// Color, tested against depth but not writing it, so what is behind still shows.
    SeeThrough = 1,
    /// Depth only. A faded object draws every mesh this way first, so that its color pass shows
    /// only its nearest surface at each pixel.
    DepthOnly = 2,
    /// A faded object's color, tested against its depth, and blended at most once per sample:
    /// the first fragment marks the stencil with the object's reference, which blocks the rest.
    /// Without it, a stroke that crosses itself blends twice where it crosses, because both of
    /// its triangles there have the nearest depth.
    FadedColor = 3,
}

/// What `draw` blends after every opaque mesh, farthest from the camera first.
enum Layer<'a> {
    /// A mesh whose material is see-through, of an object with opacity 1. Its own far side
    /// shows through it, as glass does.
    SeeThroughMesh(&'a StagedObject, &'a Mesh),
    /// An object with opacity between 0 and 1, drawn as one faded picture: its depth first, then
    /// its color at its opacity, edges included.
    Faded(&'a StagedObject, &'a [Mesh]),
    /// One lit group's shell, drawn as a faded object is: its depth first, so that only the
    /// nearest surface of all its balls and sticks shows at each sample, then its color, blended
    /// once per sample wherever it lies in front of what is solid. It follows the object's own
    /// layer, because the sort is stable and both are at the object's distance.
    Glow(&'a StagedObject, &'a StagedGlow),
}

/// The kind of a GPU, as its driver reports it, in the order a command prefers them when no
/// `--gpu` names one. `--gpu` names one, as `--gpu integrated`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum GpuKind {
    /// A GPU of its own, apart from the processor, such as an NVIDIA RTX.
    Discrete,
    /// A GPU built into the processor, such as an Intel Iris Xe or an AMD Radeon 610M.
    Integrated,
    /// A driver that names no kind, as OpenGL drivers do.
    Other,
    /// A GPU a virtual machine is given.
    Virtual,
    /// A driver that draws on the processor, such as lavapipe.
    Cpu,
}

impl GpuKind {
    fn of(kind: wgpu::DeviceType) -> GpuKind {
        match kind {
            wgpu::DeviceType::DiscreteGpu => GpuKind::Discrete,
            wgpu::DeviceType::IntegratedGpu => GpuKind::Integrated,
            wgpu::DeviceType::Other => GpuKind::Other,
            wgpu::DeviceType::VirtualGpu => GpuKind::Virtual,
            wgpu::DeviceType::Cpu => GpuKind::Cpu,
        }
    }

    fn word(self) -> &'static str {
        match self {
            GpuKind::Discrete => "discrete",
            GpuKind::Integrated => "integrated",
            GpuKind::Other => "other",
            GpuKind::Virtual => "virtual",
            GpuKind::Cpu => "cpu",
        }
    }
}

/// The GPU a command draws on, as its report names it.
#[derive(Clone, Debug, Serialize)]
pub struct GpuIdentity {
    pub name: String,
    pub kind: GpuKind,
    /// The graphics API it is drawn through, such as `vulkan` or `gl`.
    pub backend: &'static str,
    /// The driver's name, such as `NVIDIA` or `Intel open-source Mesa driver`.
    pub driver: String,
}

impl GpuIdentity {
    fn of(info: &wgpu::AdapterInfo) -> GpuIdentity {
        GpuIdentity {
            name: info.name.clone(),
            kind: GpuKind::of(info.device_type),
            backend: info.backend.to_str(),
            driver: info.driver.clone(),
        }
    }
}

#[derive(Debug)]
pub enum GpuError {
    NoGpu(String),
    /// `--gpu` names a kind no GPU of this machine has. It holds every GPU the machine has.
    NoGpuOfKind(GpuKind, Vec<GpuIdentity>),
    Device(String),
    Readback(String),
}

impl GpuError {
    pub fn message(&self) -> String {
        match self {
            GpuError::NoGpu(e) => {
                format!("no GPU can draw off screen on this machine: {e}")
            }
            GpuError::NoGpuOfKind(kind, gpus) => {
                let gpus: Vec<String> = gpus
                    .iter()
                    .map(|gpu| format!("{} ({}, {})", gpu.name, gpu.kind.word(), gpu.backend))
                    .collect();
                let has = if gpus.is_empty() {
                    "no GPU at all".to_owned()
                } else {
                    gpus.join("; ")
                };
                format!(
                    "--gpu {} names no GPU of this machine, which has {has}",
                    kind.word()
                )
            }
            GpuError::Device(e) => format!("the GPU device failed: {e}"),
            GpuError::Readback(e) => format!("reading the frame back from the GPU failed: {e}"),
        }
    }
}

/// Matches `Uniforms` in `draw.wgsl`.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    world_to_clip: [[f32; 4]; 4],
    object_to_world: [[f32; 4]; 4],
    normal_to_world: [[f32; 4]; 4],
    color: [f32; 4],
    camera_direction: [f32; 3],
    lighting: u32,
    key_light: [f32; 3],
    _padding: f32,
}

impl Gpu {
    /// Draws on the first GPU of `kind`, or with no kind on the first GPU of the earliest kind
    /// the machine has, so a discrete GPU before an integrated one.
    pub fn new(background: Background, kind: Option<GpuKind>) -> Result<Gpu, GpuError> {
        let color_format = match background {
            Background::Opaque(_) => wgpu::TextureFormat::Rgba8UnormSrgb,
            Background::Transparent => wgpu::TextureFormat::Rgba16Float,
        };
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = match kind {
            // wgpu ranks high performance as GpuKind does: discrete first, cpu last.
            // WGPU_POWER_PREF still overrides it.
            None => pollster::block_on(
                instance.request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::from_env()
                        .unwrap_or(wgpu::PowerPreference::HighPerformance),
                    ..Default::default()
                }),
            )
            .map_err(|e| GpuError::NoGpu(e.to_string()))?,
            Some(kind) => {
                let adapters =
                    pollster::block_on(instance.enumerate_adapters(wgpu::Backends::all()));
                let identities: Vec<GpuIdentity> = adapters
                    .iter()
                    .map(|adapter| GpuIdentity::of(&adapter.get_info()))
                    .collect();
                // wgpu lists Vulkan's GPUs before OpenGL's.
                match identities.iter().position(|gpu| gpu.kind == kind) {
                    Some(index) => adapters.into_iter().nth(index).expect("found in adapters"),
                    None => return Err(GpuError::NoGpuOfKind(kind, identities)),
                }
            }
        };
        let identity = GpuIdentity::of(&adapter.get_info());
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
                .map_err(|e| GpuError::Device(e.to_string()))?;
        let module = device.create_shader_module(wgpu::include_wgsl!("draw.wgsl"));
        let uniforms_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&uniforms_layout)],
            immediate_size: 0,
        });
        let pipeline = |pass: Pass, overlay: bool| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("point_main"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: size_of::<Point>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3],
                    })],
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    // A surface behind a see-through one still shows through it.
                    depth_write_enabled: Some(matches!(pass, Pass::Opaque | Pass::DepthOnly)),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: match pass {
                        Pass::FadedColor => {
                            let once = wgpu::StencilFaceState {
                                compare: wgpu::CompareFunction::NotEqual,
                                fail_op: wgpu::StencilOperation::Keep,
                                depth_fail_op: wgpu::StencilOperation::Keep,
                                pass_op: wgpu::StencilOperation::Replace,
                            };
                            wgpu::StencilState {
                                front: once,
                                back: once,
                                read_mask: 0xff,
                                write_mask: 0xff,
                            }
                        }
                        Pass::Opaque | Pass::SeeThrough | Pass::DepthOnly => Default::default(),
                    },
                    // Two triangles in one tilted plane interpolate slightly different depths, so
                    // without a bias toward the camera a fill speckles through its stroke.
                    bias: if overlay {
                        wgpu::DepthBiasState {
                            constant: -8,
                            slope_scale: -2.0,
                            clamp: 0.0,
                        }
                    } else {
                        Default::default()
                    },
                }),
                multisample: wgpu::MultisampleState {
                    count: SAMPLES,
                    ..Default::default()
                },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some("surface_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: color_format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: match pass {
                            Pass::DepthOnly => wgpu::ColorWrites::empty(),
                            Pass::Opaque | Pass::SeeThrough | Pass::FadedColor => {
                                wgpu::ColorWrites::ALL
                            }
                        },
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let pipelines = [
            Pass::Opaque,
            Pass::SeeThrough,
            Pass::DepthOnly,
            Pass::FadedColor,
        ]
        .map(|pass| [false, true].map(|overlay| pipeline(pass, overlay)));
        let (readback_sender, unused_readbacks) = std::sync::mpsc::channel();
        Ok(Gpu {
            identity,
            device,
            queue,
            uniforms_layout,
            color_format,
            pipelines,
            attachments: Mutex::new(None),
            unused_readbacks: Mutex::new(unused_readbacks),
            readback_sender,
        })
    }

    /// Opaque meshes of objects with opacity 1 go first, in draw list order, each object's edges
    /// after its meshes. Layers follow, farthest first: each see-through mesh of an object with
    /// opacity 1, and each faded object as a whole. An object with opacity 0 is not drawn.
    pub fn draw(
        &self,
        scene_config: &SceneConfig,
        draw_list: &DrawList,
    ) -> Result<Drawing, GpuError> {
        let (width, height) = (scene_config.width.get(), scene_config.height.get());
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let mut attachments = self.attachments.lock().expect("no draw panicked");
        if attachments
            .as_ref()
            .is_none_or(|attachments| attachments.size != size)
        {
            let attachment = |format, sample_count, usage| {
                self.device.create_texture(&wgpu::TextureDescriptor {
                    label: None,
                    size,
                    mip_level_count: 1,
                    sample_count,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage,
                    view_formats: &[],
                })
            };
            *attachments = Some(Attachments {
                size,
                samples: attachment(
                    self.color_format,
                    SAMPLES,
                    wgpu::TextureUsages::RENDER_ATTACHMENT,
                ),
                depth: attachment(
                    DEPTH_FORMAT,
                    SAMPLES,
                    wgpu::TextureUsages::RENDER_ATTACHMENT,
                ),
                resolved: attachment(
                    self.color_format,
                    1,
                    wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                ),
            });
        }
        let Attachments {
            samples,
            depth,
            resolved,
            ..
        } = attachments.as_ref().expect("made above");
        let bytes_per_pixel = self
            .color_format
            .block_copy_size(None)
            .expect("an RGBA color format");
        let bytes_per_row = width * bytes_per_pixel;
        let padded_bytes_per_row =
            bytes_per_row.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let readback_size = u64::from(padded_bytes_per_row) * u64::from(height);
        let readback = self
            .unused_readbacks
            .lock()
            .expect("no draw panicked")
            .try_recv()
            .ok()
            .filter(|buffer| buffer.size() == readback_size)
            .unwrap_or_else(|| {
                self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: None,
                    size: readback_size,
                    usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                })
            });

        let camera = &draw_list.camera;
        let world_to_clip = camera.world_to_clip(width as f32 / height as f32);
        let edges: Vec<Vec<Mesh>> = draw_list
            .objects
            .iter()
            .map(|object| match object.opacity {
                0.0 => Vec::new(),
                _ => object
                    .meshes
                    .iter()
                    .filter_map(|mesh| {
                        mesh::edge_mesh(mesh, object.mesh_to_world, camera, scene_config)
                    })
                    .collect(),
            })
            .collect();
        let mut opaque = Vec::new();
        let mut layers = Vec::new();
        let mut glow_layers = Vec::new();
        for (object, edges) in draw_list.objects.iter().zip(&edges) {
            if object.opacity > 0.0 {
                // After the object's own layer, if it has one: the sort below is stable.
                glow_layers.extend(object.glows.iter().map(|glow| Layer::Glow(object, glow)));
            }
            match object.opacity {
                0.0 => {}
                1.0 => {
                    object
                        .meshes
                        .iter()
                        .chain(edges)
                        .for_each(|mesh| match mesh.is_see_through() {
                            false => opaque.push((object, mesh)),
                            true => layers.push(Layer::SeeThroughMesh(object, mesh)),
                        })
                }
                _ => layers.push(Layer::Faded(object, edges)),
            }
        }
        layers.extend(glow_layers);
        layers.sort_by(|a, b| {
            let distance =
                |layer: &Layer| layer.object().transform.position.distance(camera.position);
            distance(b).total_cmp(&distance(a))
        });
        let draw = |pass: Pass, object: &StagedObject, mesh: &Mesh| {
            let pipeline = &self.pipelines[pass as usize][usize::from(mesh.overlay)];
            (pipeline, self.upload(object, mesh, world_to_clip, camera))
        };
        let mut uploads: Vec<_> = opaque
            .into_iter()
            .map(|(object, mesh)| (0, draw(Pass::Opaque, object, mesh)))
            .collect();
        let mut faded = 0;
        for layer in &layers {
            match layer {
                Layer::SeeThroughMesh(object, mesh) => {
                    uploads.push((0, draw(Pass::SeeThrough, object, mesh)))
                }
                Layer::Glow(object, glow) => {
                    let reference = faded % 255 + 1;
                    faded += 1;
                    uploads.extend(
                        glow.meshes
                            .iter()
                            .map(|mesh| (0, draw(Pass::DepthOnly, object, mesh))),
                    );
                    uploads.extend(
                        glow.meshes
                            .iter()
                            .map(|mesh| (reference, draw(Pass::FadedColor, object, mesh))),
                    );
                }
                Layer::Faded(object, edges) => {
                    // Each faded object marks the stencil with its own reference, 1 to 255, so
                    // it blocks only its own second fragments. Past 255 faded objects a
                    // reference repeats, and an object can lose samples where it overlaps the
                    // one 255 faded objects before it.
                    let reference = faded % 255 + 1;
                    faded += 1;
                    uploads.extend(
                        object
                            .meshes
                            .iter()
                            .map(|mesh| (0, draw(Pass::DepthOnly, object, mesh))),
                    );
                    // Edges first: they lie just in front of the faces, and the stencil lets only
                    // the first fragment of each sample through.
                    uploads.extend(
                        edges
                            .iter()
                            .chain(&object.meshes)
                            .map(|mesh| (reference, draw(Pass::FadedColor, object, mesh))),
                    );
                }
            }
        }

        let [r, g, b, a] = match scene_config.background {
            Background::Opaque(color) => color.linear(1.0),
            Background::Transparent => [0.0; 4],
        }
        .map(f64::from);
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let samples_attachment = samples.create_view(&Default::default());
            let resolved_attachment = resolved.create_view(&Default::default());
            let depth_attachment = depth.create_view(&Default::default());
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &samples_attachment,
                    depth_slice: None,
                    resolve_target: Some(&resolved_attachment),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r, g, b, a }),
                        store: wgpu::StoreOp::Discard,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_attachment,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0),
                        store: wgpu::StoreOp::Discard,
                    }),
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            for (stencil_reference, (pipeline, upload)) in &uploads {
                pass.set_pipeline(pipeline);
                pass.set_stencil_reference(*stencil_reference);
                pass.set_bind_group(0, &upload.uniforms, &[]);
                pass.set_vertex_buffer(0, upload.points.slice(..));
                pass.set_index_buffer(upload.triangles.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..upload.index_count, 0, 0..1);
            }
        }
        encoder.copy_texture_to_buffer(
            resolved.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: None,
                },
            },
            size,
        );
        let submission = self.queue.submit([encoder.finish()]);

        let (sender, mapped) = std::sync::mpsc::channel();
        readback.map_async(wgpu::MapMode::Read, .., move |result| {
            let _ = sender.send(result);
        });
        Ok(Drawing {
            readback: Readback {
                buffer: readback,
                width,
                height,
                bytes_per_row,
                padded_bytes_per_row,
                color_format: self.color_format,
                unused: self.readback_sender.clone(),
            },
            submission,
            mapped,
        })
    }

    /// Waits for `drawing` alone, so a frame drawn after it can still be on the GPU.
    pub fn readback(&self, drawing: Drawing) -> Result<Readback, GpuError> {
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(drawing.submission),
                timeout: None,
            })
            .map_err(|e| GpuError::Device(e.to_string()))?;
        drawing
            .mapped
            .recv()
            .map_err(|e| GpuError::Readback(e.to_string()))?
            .map_err(|e| GpuError::Readback(e.to_string()))?;
        Ok(drawing.readback)
    }

    /// An edge mesh is already in world space.
    fn upload(
        &self,
        object: &StagedObject,
        mesh: &Mesh,
        world_to_clip: Mat4,
        camera: &Camera,
    ) -> Upload {
        let object_to_world = if mesh.lighting == Lighting::Unlit {
            Mat4::IDENTITY
        } else {
            object.mesh_to_world
        };
        let [r, g, b, a] = mesh.material.linear_rgba();
        let uniforms = Uniforms {
            world_to_clip: world_to_clip.to_cols_array_2d(),
            object_to_world: object_to_world.to_cols_array_2d(),
            normal_to_world: Mat4::from_mat3(cofactor(Mat3::from_mat4(object_to_world)))
                .to_cols_array_2d(),
            color: [r, g, b, a * object.opacity],
            camera_direction: camera.direction().to_array(),
            lighting: mesh.lighting as u32,
            key_light: camera.key_light().to_array(),
            _padding: 0.0,
        };
        let buffer = |contents: &[u8], usage| {
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: None,
                    contents,
                    usage,
                })
        };
        let uniforms_buffer = buffer(bytemuck::bytes_of(&uniforms), wgpu::BufferUsages::UNIFORM);
        Upload {
            uniforms: self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &self.uniforms_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniforms_buffer.as_entire_binding(),
                }],
            }),
            points: buffer(
                bytemuck::cast_slice(&mesh.points),
                wgpu::BufferUsages::VERTEX,
            ),
            triangles: buffer(
                bytemuck::cast_slice(&mesh.triangles_placed_by(object_to_world)),
                wgpu::BufferUsages::INDEX,
            ),
            index_count: mesh.triangles.len() as u32 * 3,
        }
    }
}

/// A frame submitted to the GPU, with its readback not yet mapped.
pub struct Drawing {
    readback: Readback,
    submission: wgpu::SubmissionIndex,
    mapped: Receiver<Result<(), wgpu::BufferAsyncError>>,
}

/// A drawn frame still in the GPU's color format, in a buffer mapped for reading, its rows padded
/// to wgpu's copy alignment. It holds no borrow of the `Gpu`, so another thread can turn it into
/// a `Frame` while the GPU draws the next one.
pub struct Readback {
    buffer: wgpu::Buffer,
    width: u32,
    height: u32,
    bytes_per_row: u32,
    padded_bytes_per_row: u32,
    color_format: wgpu::TextureFormat,
    unused: Sender<wgpu::Buffer>,
}

/// The textures `Gpu::draw` draws a frame into, at one frame size.
struct Attachments {
    size: wgpu::Extent3d,
    samples: wgpu::Texture,
    depth: wgpu::Texture,
    resolved: wgpu::Texture,
}

impl Readback {
    pub fn frame(self) -> Result<Frame, GpuError> {
        let mapped = self
            .buffer
            .get_mapped_range(..)
            .map_err(|e| GpuError::Readback(e.to_string()))?;
        let mut rgba = Vec::with_capacity(self.width as usize * self.height as usize * 4);
        for row in mapped.chunks_exact(self.padded_bytes_per_row as usize) {
            let row = &row[..self.bytes_per_row as usize];
            match self.color_format {
                wgpu::TextureFormat::Rgba8UnormSrgb => rgba.extend_from_slice(row),
                wgpu::TextureFormat::Rgba16Float => {
                    for pixel in row.chunks_exact(8) {
                        rgba.extend_from_slice(&straight_rgba(pixel));
                    }
                }
                _ => unreachable!("GPU color format is chosen in Gpu::new"),
            }
        }
        drop(mapped);
        self.buffer.unmap();
        let _ = self.unused.send(self.buffer);
        Ok(Frame {
            width: self.width,
            height: self.height,
            rgba,
        })
    }
}

fn straight_rgba(pixel: &[u8]) -> [u8; 4] {
    let component =
        |index: usize| half::f16::from_le_bytes([pixel[index * 2], pixel[index * 2 + 1]]).to_f32();
    let pixel_alpha = component(3);
    let pixel_alpha_byte = (pixel_alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
    if pixel_alpha_byte == 0 {
        return [0; 4];
    }
    let color = |index| {
        let linear = (component(index) / pixel_alpha).clamp(0.0, 1.0);
        let srgb = if linear <= 0.0031308 {
            linear * 12.92
        } else {
            1.055 * linear.powf(1.0 / 2.4) - 0.055
        };
        (srgb * 255.0).round() as u8
    };
    [color(0), color(1), color(2), pixel_alpha_byte]
}

impl Layer<'_> {
    fn object(&self) -> &StagedObject {
        match self {
            Layer::SeeThroughMesh(object, _) | Layer::Faded(object, _) | Layer::Glow(object, _) => {
                object
            }
        }
    }
}

struct Upload {
    uniforms: wgpu::BindGroup,
    points: wgpu::Buffer,
    triangles: wgpu::Buffer,
    index_count: u32,
}

/// The inverse transpose scaled by the determinant, negated when the determinant is negative. It
/// turns normals the right way under any scale, including a zero scale on one axis that flattens
/// a sphere into a disc, and keeps them pointing out of a solid that a negative scale mirrors, so
/// the key light lights its outside.
pub(crate) fn cofactor(m: Mat3) -> Mat3 {
    let cofactor = Mat3::from_cols(
        m.y_axis.cross(m.z_axis),
        m.z_axis.cross(m.x_axis),
        m.x_axis.cross(m.y_axis),
    );
    if m.determinant() < 0.0 {
        -cofactor
    } else {
        cofactor
    }
}
