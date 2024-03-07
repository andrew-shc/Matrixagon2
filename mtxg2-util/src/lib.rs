pub use matrixagon_derive::Vertex;


pub trait VulkanVertexState<const A: usize> {
    const BINDING_DESCRIPTION: ash::vk::VertexInputBindingDescription;
    const ATTRIBUTE_DESCRIPTION: [ash::vk::VertexInputAttributeDescription; A];
    const VERTEX_INPUT_STATE: ash::vk::PipelineVertexInputStateCreateInfo;
}


#[macro_export]
macro_rules! create_renderpass {
    {
        [$device:expr];
        Attachments {$(
            $attachment_names:ident: {
                format: $format:expr,
                samples: $samples:ident,
                load: $load:ident,
                store: $store:ident,
                stencil_load: $stencil_load:ident,
                stencil_store: $stencil_store:ident,
                initial: $initial:ident,
                final: $final:ident,
            }
        )*}
        Subpasses {$(
            $subpass_names:ident: {
                input: $($input_attachment_ref:ident~$input_attachment_ref_layout:ident)*,
                color: $($color_attachment_ref:ident~$color_attachment_ref_layout:ident)*,
                resolve: $($resolve_attachment_ref:ident~$resolve_attachment_ref_layout:ident)*,
                preserve: $($preserve_attachment_ref:ident)*,
                depth: $($depth_attachment_ref:ident~$depth_attachment_ref_layout:ident)?,
            }
        )*}
        Dependencies {$(
            $($subpass_in:ident)?->$($subpass_out:ident)?: {
                src_stage: $($src_stage_flags:ident)|*,
                dst_stage:  $($dst_stage_flags:ident)|*,
                src_access: $($src_access_flags:ident)|*,
                dst_access: $($dst_access_flags:ident)|*,
            }
        )*}
    } => {{
        let mut __attachment_count = 0;
        $(
            let $attachment_names = (
                __attachment_count,
                ash::vk::AttachmentDescription {
                    format: $format,
                    samples: ash::vk::SampleCountFlags::$samples,
                    load_op: ash::vk::AttachmentLoadOp::$load, store_op: ash::vk::AttachmentStoreOp::$store,
                    stencil_load_op: ash::vk::AttachmentLoadOp::$stencil_load, stencil_store_op: ash::vk::AttachmentStoreOp::$stencil_store,
                    initial_layout: ash::vk::ImageLayout::$initial,
                    final_layout: ash::vk::ImageLayout::$final,
                    ..Default::default()
                }
            );
            __attachment_count += 1;
        )*

        let mut __subpass_count = 0;
        $(
            let mut __input = create_renderpass!(@ATTACHMENT_REFS $($input_attachment_ref~$input_attachment_ref_layout)*);
            let mut __color = create_renderpass!(@ATTACHMENT_REFS $($color_attachment_ref~$color_attachment_ref_layout)*);
            let mut __resolve = create_renderpass!(@ATTACHMENT_REFS $($resolve_attachment_ref~$resolve_attachment_ref_layout)*);
            let mut __preserve = create_renderpass!(@PRESERVE_ATTACHMENTS $($preserve_attachment_ref)*);
            $(
            let __depth = create_renderpass!(@SINGLE_ATTACHMENT_REF $depth_attachment_ref~$depth_attachment_ref_layout);
            )?
            let $subpass_names = (
                __subpass_count,
                {
                    let mut subpass_builder = ash::vk::SubpassDescription::builder()
                        .pipeline_bind_point(ash::vk::PipelineBindPoint::GRAPHICS);
                    if __input.len() > 0 {
                        subpass_builder = subpass_builder.input_attachments(&__input)
                    }
                    if __color.len() > 0 {
                        subpass_builder = subpass_builder.color_attachments(&__color)
                    }
                    if __resolve.len() > 0 {
                        subpass_builder = subpass_builder.resolve_attachments(&__resolve)
                    }
                    if __preserve.len() > 0 {
                        subpass_builder = subpass_builder.preserve_attachments(&__preserve)
                    }
                    subpass_builder
                    $(
                    .depth_stencil_attachment({$depth_attachment_ref; &__depth})
                    )?
                    .build()
                }

            );

            __subpass_count += 1;
        )*

        let __dependencies = vec![$(
        {
            let mut __src_subpass = ash::vk::SUBPASS_EXTERNAL;
            let mut __dst_subpass = ash::vk::SUBPASS_EXTERNAL;
            $(
            __src_subpass = $subpass_in.0;  // subpass_in should refer to a subpass variable
            )?
            $(
            __dst_subpass = $subpass_out.0;  // subpass_out should refer to a subpass variable
            )?

            vk::SubpassDependency {
                src_subpass: __src_subpass, dst_subpass: __dst_subpass,
                src_stage_mask: create_renderpass!(@STAGE_FLAGS $($src_stage_flags)|*),
                dst_stage_mask: create_renderpass!(@STAGE_FLAGS $($dst_stage_flags)|*),
                src_access_mask: create_renderpass!(@ACCESS_FLAGS $($src_access_flags)|*),
                dst_access_mask: create_renderpass!(@ACCESS_FLAGS $($dst_access_flags)|*),
                ..Default::default()
            }
        },
        )*];

        let __attachments = vec![$($attachment_names.1),*];
        let __subpasses = vec![$($subpass_names.1),*];

        let __renderpass_info = ash::vk::RenderPassCreateInfo::builder()
            .attachments(&__attachments)
            .subpasses(&__subpasses)
            .dependencies(&__dependencies)
            .build();
        $device.create_render_pass(&__renderpass_info, None).unwrap()
    }};
    (@ATTACHMENT_REFS $($attachment_ref:ident~$attachment_ref_layout:ident)*) => {{
        [$(
           ash::vk::AttachmentReference {
               attachment: $attachment_ref.0,
               layout: ash::vk::ImageLayout::$attachment_ref_layout
           }
        ),*]
    }};
    (@SINGLE_ATTACHMENT_REF $attachment_ref:ident~$attachment_ref_layout:ident) => {{
       ash::vk::AttachmentReference {
           attachment: $attachment_ref.0,
           layout: ash::vk::ImageLayout::$attachment_ref_layout
       }
    }};
    (@PRESERVE_ATTACHMENTS $($attachment_ref:ident)*) => {{
        [$($attachment_ref.0),*]
    }};
    (@STAGE_FLAGS $($stage_flags:ident)|*) => {{
        ash::vk::PipelineStageFlags::empty() $(| ash::vk::PipelineStageFlags::$stage_flags)*
    }};
    (@ACCESS_FLAGS $($access_flags:ident)|*) => {{
        ash::vk::AccessFlags::empty() $(| ash::vk::AccessFlags::$access_flags)*
    }};
}
