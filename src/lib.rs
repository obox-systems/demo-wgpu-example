use std::iter;

#[cfg(target_arch="wasm32")]
use wasm_bindgen::prelude::*;

use wgpu::util::DeviceExt;
use winit::{
  event::*,
  event_loop::{ControlFlow, EventLoop},
  window::WindowBuilder,
};

use winit::window::Window;

struct State {
  surface: wgpu::Surface,
  device: wgpu::Device,
  queue: wgpu::Queue,
  config: wgpu::SurfaceConfiguration,
  size: winit::dpi::PhysicalSize<u32>,
  window: Window,
  clear_color: wgpu::Color,
  render_pipeline: wgpu::RenderPipeline,
  challenge_render_pipeline: wgpu::RenderPipeline,
  use_color: bool,
  vertex_buffer: wgpu::Buffer,
  num_vertices: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
  position: [f32; 3],
  color: [f32; 3],
}

impl Vertex {
  const ATTRIBS: [wgpu::VertexAttribute; 2] =
    wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3];

  fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
    use std::mem;

    wgpu::VertexBufferLayout {
      array_stride: mem::size_of::<Self>() as wgpu::BufferAddress,
      step_mode: wgpu::VertexStepMode::Vertex,
      attributes: &Self::ATTRIBS,
    }
  }
}

const VERTICES: &[Vertex] = &[
  Vertex { position: [0.0, 0.5, 0.0], color: [1.0, 0.0, 0.0] },
  Vertex { position: [-0.5, -0.5, 0.0], color: [0.0, 1.0, 0.0] },
  Vertex { position: [0.5, -0.5, 0.0], color: [0.0, 0.0, 1.0] },
];

impl State {
  // Creating some of the wgpu types requires async code
  async fn new(window: Window) -> Self {
    let size = window.inner_size();

    // The instance is a handle to our GPU
    // Backends::all => Vulkan + Metal + DX12 + Browser WebGPU
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        dx12_shader_compiler: Default::default(),
    });
    
    // # Safety
    //
    // The surface needs to live as long as the window that created it.
    // State owns the window so this should be safe.
    let surface = unsafe { instance.create_surface(&window) }.unwrap();

    let adapter = instance
    .enumerate_adapters(wgpu::Backends::all())
    .filter(|adapter| {
        // Check if this adapter supports our surface
        adapter.is_surface_supported(&surface)
    })
    .next()
    .unwrap();

    let (device, queue) = adapter.request_device(
      &wgpu::DeviceDescriptor {
        features: wgpu::Features::empty(),
        // WebGL doesn't support all of wgpu's features, so if
        // we're building for the web we'll have to disable some.
        limits: if cfg!(target_arch = "wasm32") {
          wgpu::Limits::downlevel_webgl2_defaults()
        } else {
          wgpu::Limits::default()
        },
        label: None,
      },
      None, // Trace path
    ).await.unwrap();

    let surface_caps = surface.get_capabilities(&adapter);
    // Shader code in this tutorial assumes an sRGB surface texture. Using a different
    // one will result all the colors coming out darker. If you want to support non
    // sRGB surfaces, you'll need to account for that when drawing to the frame.
    let surface_format = surface_caps.formats.iter()
      .copied()
      .filter(|f| f.describe().srgb)
      .next()
      .unwrap_or(surface_caps.formats[0]);
    let config = wgpu::SurfaceConfiguration {
      usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
      format: surface_format,
      width: size.width,
      height: size.height,
      present_mode: surface_caps.present_modes[0],
      alpha_mode: surface_caps.alpha_modes[0],
      view_formats: vec![],
    };
    surface.configure(&device, &config);

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
      label: Some("Shader"),
      source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
    });

    let render_pipeline_layout =
    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Render Pipeline Layout"),
        bind_group_layouts: &[],
        push_constant_ranges: &[],
    });

    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
      label: Some("Render Pipeline"),
      layout: Some(&render_pipeline_layout),
      vertex: wgpu::VertexState {
          module: &shader,
          entry_point: "vs_main", // 1.
          buffers: &[
            Vertex::desc(),
          ],// 2.
      },
      fragment: Some(wgpu::FragmentState { // 3.
          module: &shader,
          entry_point: "fs_main",
          targets: &[Some(wgpu::ColorTargetState { // 4.
              format: config.format,
              blend: Some(wgpu::BlendState::REPLACE),
              write_mask: wgpu::ColorWrites::ALL,
          })],
      }),
      primitive: wgpu::PrimitiveState {
        topology: wgpu::PrimitiveTopology::TriangleList, // 1.
        strip_index_format: None,
        front_face: wgpu::FrontFace::Ccw, // 2.
        cull_mode: Some(wgpu::Face::Back),
        // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
        polygon_mode: wgpu::PolygonMode::Fill,
        // Requires Features::DEPTH_CLIP_CONTROL
        unclipped_depth: false,
        // Requires Features::CONSERVATIVE_RASTERIZATION
        conservative: false,
      },
      depth_stencil: None, // 1.
      multisample: wgpu::MultisampleState {
        count: 1, // 2.
        mask: !0, // 3.
        alpha_to_coverage_enabled: false, // 4.
      },
      multiview: None, // 5.
    });

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
      label: Some("Challenge Shader"),
      source: wgpu::ShaderSource::Wgsl(include_str!("challenge.wgsl").into()),
    });

    let challenge_render_pipeline =
      device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Render Pipeline"),
        layout: Some(&render_pipeline_layout),
        vertex: wgpu::VertexState {
          module: &shader,
          entry_point: "vs_main",
          buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
          module: &shader,
          entry_point: "fs_main",
          targets: &[Some(wgpu::ColorTargetState {
              format: config.format,
              blend: Some(wgpu::BlendState::REPLACE),
              write_mask: wgpu::ColorWrites::ALL,
          })],
        }),
        primitive: wgpu::PrimitiveState {
          topology: wgpu::PrimitiveTopology::TriangleList,
          strip_index_format: None,
          front_face: wgpu::FrontFace::Ccw,
          cull_mode: Some(wgpu::Face::Back),
          // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
          polygon_mode: wgpu::PolygonMode::Fill,
          ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
          count: 1,
          mask: !0,
          alpha_to_coverage_enabled: false,
        },
        // If the pipeline will be used with a multiview render pass, this
        // indicates how many array layers the attachments will have.
        multiview: None,
    });

    let use_color = true;

    let vertex_buffer = device.create_buffer_init(
      &wgpu::util::BufferInitDescriptor {
          label: Some("Vertex Buffer"),
          contents: bytemuck::cast_slice(VERTICES),
          usage: wgpu::BufferUsages::VERTEX,
      }
    );

    let num_vertices = VERTICES.len() as u32;

    Self {
      window,
      surface,
      device,
      queue,
      config,
      size,
      clear_color: wgpu::Color::BLACK,
      render_pipeline,
      challenge_render_pipeline,
      use_color,
      vertex_buffer,
      num_vertices,
    }
  }

  pub fn window(&self) -> &Window {
      &self.window
  }

  fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
    if new_size.width > 0 && new_size.height > 0 {
      self.size = new_size;
      self.config.width = new_size.width;
      self.config.height = new_size.height;
      self.surface.configure(&self.device, &self.config);
    }
  }

  fn input(&mut self, event: &WindowEvent) -> bool {
    match event {
      WindowEvent::CursorMoved { position, .. } => {
        self.clear_color = wgpu::Color {
          r: position.x as f64 / self.size.width as f64,
          g: position.y as f64 / self.size.height as f64,
          b: (position.x as f64 * position.y as f64) / (self.size.width as f64 *self.size.height as f64) ,
          a: 1.0,
        };
        true
      },
      WindowEvent::KeyboardInput {
        input:
          KeyboardInput {
            state,
            virtual_keycode: Some(VirtualKeyCode::Space),
            ..
          },
        ..
      } => {
        self.use_color = *state == ElementState::Released;
        true
      },
      _ => false,
    }
  }

  fn update(&mut self) {
      //todo!()
  }

  fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
    let output = self.surface.get_current_texture()?;
    let view = output
      .texture
      .create_view(&wgpu::TextureViewDescriptor::default());

    let mut encoder = self
      .device
      .create_command_encoder(&wgpu::CommandEncoderDescriptor {
          label: Some("Render Encoder"),
      });

    {
      let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Render Pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
          view: &view,
          resolve_target: None,
          ops: wgpu::Operations {
            load: wgpu::LoadOp::Clear(self.clear_color),
            store: true,
          },
        })],
        depth_stencil_attachment: None,
      });

      render_pass.set_pipeline(if self.use_color {
        &self.render_pipeline
      } else {
        &self.challenge_render_pipeline
      });
      render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
      render_pass.draw(0..self.num_vertices, 0..1);
    }

    self.queue.submit(iter::once(encoder.finish()));
    output.present();

    Ok(())
  }
}

#[cfg_attr(target_arch="wasm32", wasm_bindgen(start))]
pub async fn run() {
  cfg_if::cfg_if! {
    if #[cfg(target_arch = "wasm32")] {
      std::panic::set_hook(Box::new(console_error_panic_hook::hook));
      console_log::init_with_level(log::Level::Warn).expect("Couldn't initialize logger");
    } else {
      env_logger::init();
    }
  }

  
  let event_loop = EventLoop::new();
  let window = WindowBuilder::new().build(&event_loop).unwrap();

  #[cfg(target_arch = "wasm32")]
  {
    // Winit prevents sizing with CSS, so we have to set
    // the size manually when on web.
    use winit::dpi::PhysicalSize;
    window.set_inner_size(PhysicalSize::new(450, 400));
    
    use winit::platform::web::WindowExtWebSys;
    web_sys::window()
        .and_then(|win| win.document())
        .and_then(|doc| {
            let dst = doc.get_element_by_id("wasm-example")?;
            let canvas = web_sys::Element::from(window.canvas());
            dst.append_child(&canvas).ok()?;
            Some(())
        })
        .expect("Couldn't append canvas to document body.");
  }

  let mut state = State::new(window).await;

  event_loop.run(move |event, _, control_flow| {
    match event {
        Event::WindowEvent {
            ref event,
            window_id,
        } if window_id == state.window().id() => {
            if !state.input(event) {
                match event {
                    WindowEvent::CloseRequested
                    | WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                state: ElementState::Pressed,
                                virtual_keycode: Some(VirtualKeyCode::Escape),
                                ..
                            },
                        ..
                    } => *control_flow = ControlFlow::Exit,
                    WindowEvent::Resized(physical_size) => {
                        state.resize(*physical_size);
                    }
                    WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                        // new_inner_size is &mut so w have to dereference it twice
                        state.resize(**new_inner_size);
                    }
                    _ => {}
                }
            }
        }
        Event::RedrawRequested(window_id) if window_id == state.window().id() => {
          state.update();
          match state.render() {
            Ok(_) => {}
            // Reconfigure the surface if it's lost or outdated
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => state.resize(state.size),
            // The system is out of memory, we should probably quit
            Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
            // We're ignoring timeouts
            Err(wgpu::SurfaceError::Timeout) => log::warn!("Surface timeout"),
          }
        }
        Event::MainEventsCleared => {
            state.window().request_redraw();
        }
        _ => {}
    }
  });
}