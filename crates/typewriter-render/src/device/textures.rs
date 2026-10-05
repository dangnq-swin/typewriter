//! The textures a scene's meshes read: uploaded by name, replaced where a
//! frame brings new texels, each name keeping its handle — a mesh already
//! named on it keeps drawing what it now holds.

use std::collections::HashMap;

use super::mesh::Texture;
use super::pipelines::texture_layout;

/// The uploaded textures: group 0's texture and sampler, one bind group
/// each, as egui's are so what the app uploads reads as its own does.
pub(super) struct Textures {
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    /// The bind groups, by handle: a draw binds the texture its mesh
    /// names.
    held: Vec<wgpu::BindGroup>,
    names: HashMap<String, u32>,
}

impl Textures {
    /// The registry, with its layout and sampler and nothing uploaded yet.
    pub(super) fn new(device: &wgpu::Device) -> Self {
        Self {
            layout: texture_layout(device),
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("scene_textures"),
                // The paper's texels and the glyphs both read smoothed, as
                // egui's own textures do.
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
            held: Vec::new(),
            names: HashMap::new(),
        }
    }

    /// Uploads `rgba`'s texels — premultiplied sRGB gamma, as egui's — as
    /// `name`, and returns its handle. A name replaced keeps its handle,
    /// so meshes already named on it draw the new texels: the font atlas
    /// grows this way.
    pub(super) fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        name: String,
        size: [u32; 2],
        rgba: &[u8],
    ) -> Texture {
        let [width, height] = size;
        debug_assert!(
            width > 0 && height > 0,
            "a {width}×{height} texture draws nothing"
        );
        debug_assert_eq!(
            rgba.len(),
            (width * height * 4) as usize,
            "texels for a {width}×{height} texture"
        );
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("scene_texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        // `write_texture` needs no row alignment, unlike a buffer copy.
        queue.write_texture(
            texture.as_image_copy(),
            rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&Default::default());
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scene_texture"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        // The group holds its texture and view, so these handles end here.
        let id = match self.names.get(&name) {
            Some(&id) => {
                self.held[id as usize] = group;
                id
            }
            None => {
                let id = self.held.len() as u32;
                self.held.push(group);
                self.names.insert(name, id);
                id
            }
        };
        Texture(id)
    }

    /// The bind group a draw of `texture` binds.
    pub(super) fn group(&self, texture: Texture) -> Option<&wgpu::BindGroup> {
        self.held.get(texture.0 as usize)
    }
}
