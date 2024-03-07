extern crate proc_macro;
use proc_macro::TokenStream;

use syn::{DeriveInput, parse_macro_input, Type, TypeArray, TypePath, Expr, ExprLit, Lit, LitInt, Ident};
use quote::quote;



fn convert_into_vulkan_type(data_type: String, len: usize) -> ash::vk::Format {
    match (data_type.as_str(), len) {
        ("f32", 4) => ash::vk::Format::R32G32B32A32_SFLOAT,
        ("f32", 3) => ash::vk::Format::R32G32B32_SFLOAT,
        ("f32", 2) => ash::vk::Format::R32G32_SFLOAT,
        ("f32", 1) => ash::vk::Format::R32_SFLOAT,
        ("u8", 4) => ash::vk::Format::R8G8B8A8_UINT,
        ("u8", 3) => ash::vk::Format::R8G8B8_UINT,
        ("u8", 2) => ash::vk::Format::R8G8_UINT,
        ("u8", 1) => ash::vk::Format::R8_UINT,
        _ => unimplemented!("Vertex Derive / Type Conversion: Unknown Possible Valid Type {:?} {:?}", data_type, len)
    }
}


#[proc_macro_derive(Vertex)]
pub fn derive_vertex(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = input.ident;

    let data = if let syn::Data::Struct(data) = input.data {
        data
    } else {
        unimplemented!();
    };

    let vertex_attribute_locations = data.fields.iter().map(|f| {
        match &f.ty {
            Type::Array(TypeArray { elem, len: Expr::Lit(ExprLit {attrs: _, lit: Lit::Int(length)}), .. }) => {
                if let Type::Path(TypePath {path, ..}) = (**elem).clone() {
                    if let Some(homogenous_type) = path.get_ident() {
                        // println!("Vertex Derive / Array: {:?} {:?}", homogenous_type.to_string(), length.base10_parse::<usize>().unwrap());
                        let vk_type = convert_into_vulkan_type(homogenous_type.to_string(), length.base10_parse::<usize>().unwrap());
                        let vk_format: proc_macro2::TokenStream = format!("ash::vk::Format::{:?}", vk_type).parse().unwrap();
                        println!("Vertex Derive / Array: {:?}", vk_format);
                        let field_name = &f.ident.as_ref().expect("Field names are expected.");
                        quote! {
                            ash::vk::VertexInputAttributeDescription {
                                binding: 0u32,
                                location: 0u32,
                                format: #vk_format,
                                offset: {
                                    let f_u8_ptr = std::ptr::addr_of!((*b_ptr).#field_name) as *const u8;
                                    f_u8_ptr.offset_from(b_u8_ptr) as u32
                                },
                            }
                        }
                    } else {
                        unimplemented!("Vertex Derive / Array: Not a type");
                    }
                } else {
                    unimplemented!("Vertex Derive / Array: Not a type");
                }
            }
            Type::Path(TypePath {path, ..}) => {
                if let Some(scalar_type) = path.get_ident() {
                    // println!("Vertex Derive / Type: {:?}", scalar_type.to_string());
                    let vk_type = convert_into_vulkan_type(scalar_type.to_string(), 1);
                    let vk_format: proc_macro2::TokenStream = format!("ash::vk::Format::{:?}", vk_type).parse().unwrap();
                    println!("Vertex Derive / Array: {:?}", vk_format);
                    let field_name = &f.ident.as_ref().expect("Field names are expected.");
                    quote! {
                        ash::vk::VertexInputAttributeDescription {
                            binding: 0u32,
                            location: 0u32,
                            format: #vk_format,
                            offset: {
                                let f_u8_ptr = std::ptr::addr_of!((*b_ptr).#field_name) as *const u8;
                                f_u8_ptr.offset_from(b_u8_ptr) as u32
                            },
                        }
                    }
                } else {
                    unimplemented!("Vertex Derive / Type: Not a type");
                }
            }
            _ => {
                unimplemented!("Vertex Derive / Unknown: {:?}", &f.ty);
            }
        }
    });

    let field_count = vertex_attribute_locations.len();


    let expanded = quote! {
        impl matrixagon_util::VulkanVertexState<{#field_count}> for #name {
            const BINDING_DESCRIPTION: ash::vk::VertexInputBindingDescription = ash::vk::VertexInputBindingDescription {
                binding: 0,
                stride: std::mem::size_of::<#name>() as u32,
                input_rate: ash::vk::VertexInputRate::VERTEX,
            };

            const ATTRIBUTE_DESCRIPTION: [ash::vk::VertexInputAttributeDescription; #field_count] = unsafe {
                let b = std::mem::MaybeUninit::uninit();
                let b_ptr: *const #name = b.as_ptr();

                // cast to u8 pointers so we get offset in bytes
                let b_u8_ptr = b_ptr as *const u8;

                let mut locations = [#(#vertex_attribute_locations),*];

                let count = #field_count;
                let mut i = 0usize;
                while i < count {
                    locations[i].location = i as u32;
                    i += 1;
                }

                locations
            };

            const VERTEX_INPUT_STATE: ash::vk::PipelineVertexInputStateCreateInfo = ash::vk::PipelineVertexInputStateCreateInfo {
                s_type: ash::vk::StructureType::PIPELINE_VERTEX_INPUT_STATE_CREATE_INFO,
                p_next: std::ptr::null(),
                flags: ash::vk::PipelineVertexInputStateCreateFlags::empty(),
                vertex_binding_description_count: 1,
                p_vertex_binding_descriptions: &Self::BINDING_DESCRIPTION as *const ash::vk::VertexInputBindingDescription,
                vertex_attribute_description_count: #field_count as u32,
                p_vertex_attribute_descriptions: &Self::ATTRIBUTE_DESCRIPTION as *const ash::vk::VertexInputAttributeDescription,
            };
        }
    };

    TokenStream::from(expanded)
}

