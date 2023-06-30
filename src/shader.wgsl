struct Sprite {
    top_left_position_x: f32,
    top_left_position_y: f32,
    top_left_tex_coords_x: f32,
    top_left_tex_coords_y: f32,
    width: f32,
    height: f32,
    depth_base: f32,
    origin_offset_y: f32,
    
    frame_num: u32,
    frame_interval: f32,
    looping: u32,
}

struct Animation {
    counter: f32,
    current_frame_index: u32,
}

struct UniformData {
    height_resolution: f32,
    texture_width: f32,
    texture_height: f32,
    window_width: f32,
    window_height: f32,
    utime: f32,
    // these two are not used for right now
    _dtime: f32,
    _lasttime: f32,
}

@group(0) @binding(1) var<uniform> uniform_data: UniformData;
@group(0) @binding(2) var<storage, read_write> storage_array: array<Sprite>;
@group(0) @binding(3) var<storage, read_write> anim_storage_array: array<Animation>;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

fn gridify(pixel_val: f32) -> f32 {
    return floor(pixel_val) / uniform_data.height_resolution;
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    let index = vertex_index / 6u;
    let sprite = storage_array[index];
    let scale = uniform_data.window_height / uniform_data.window_width;
    let origin_pos_y = (sprite.top_left_position_y - sprite.origin_offset_y) / uniform_data.height_resolution;

    // updating the time
    // todo, something is wrong, pls fix this
    // var anim_x_offset: f32;
    // var anim_data = anim_storage_array[index];
    // if sprite.frame_num != 1u && sprite.frame_num != 0u {
    //     anim_data.counter += uniform_data.delta_time;
    //     if anim_data.counter >= sprite.frame_interval {
    //         if anim_data.current_frame_index <= sprite.frame_num {
    //             anim_data.current_frame_index += 1u;
    //         } else {
    //             if sprite.looping == 1u {
    //                 anim_data.current_frame_index = 0u;
    //             }
    //         }
    //     }
    // }
    // anim_x_offset = f32(anim_data.current_frame_index) * sprite.width;

    // if anim_storage_array[0].counter == 0.0 {
    //     anim_storage_array[0].counter = uniform_data.utime;
    // } else {
    //     if uniform_data.utime - anim_storage_array[0].counter > 0.5 {
    //         if anim_storage_array[0].current_frame_index == 0u {
    //             anim_storage_array[0].current_frame_index = 1u;
    //         } else {
    //             anim_storage_array[0].current_frame_index = 0u;
    //         }
    //         anim_storage_array[0].counter = 0.0;
    //     }
    // }
    // var anim_x_offset = f32(anim_storage_array[0].current_frame_index) * 32.0;
    var anim_x_offset = 0.0;


    // even tho we only have four layers we only allow 0.2 range of depth change
    let delta_depth = 0.2 * (origin_pos_y + 1.0) / 2.0;
    switch vertex_index % 6u {
        case 0u:{
            out.position = vec4<f32>(scale * gridify(sprite.top_left_position_x), gridify(sprite.top_left_position_y), sprite.depth_base + delta_depth, 1.0);
            out.tex_coords = vec2<f32>(sprite.top_left_tex_coords_x + anim_x_offset, sprite.top_left_tex_coords_y);
        }
        case 1u:{
            out.position = vec4<f32>(scale * gridify(sprite.top_left_position_x), gridify(sprite.top_left_position_y - sprite.height), sprite.depth_base + delta_depth, 1.0);
            out.tex_coords = vec2<f32>(sprite.top_left_tex_coords_x + anim_x_offset, sprite.top_left_tex_coords_y + sprite.height);
        }
        case 2u:{
            out.position = vec4<f32>(scale * gridify(sprite.top_left_position_x + sprite.width), gridify(sprite.top_left_position_y), sprite.depth_base + delta_depth, 1.0);
            out.tex_coords = vec2<f32>(sprite.top_left_tex_coords_x + sprite.width + anim_x_offset, sprite.top_left_tex_coords_y);
        }
        case 3u:{
            out.position = vec4<f32>(scale * gridify(sprite.top_left_position_x), gridify(sprite.top_left_position_y - sprite.height), sprite.depth_base + delta_depth, 1.0);
            out.tex_coords = vec2<f32>(sprite.top_left_tex_coords_x + anim_x_offset, sprite.top_left_tex_coords_y + sprite.height);
        }
        case 4u:{
            out.position = vec4<f32>(scale * gridify(sprite.top_left_position_x + sprite.width), gridify(sprite.top_left_position_y - sprite.height), sprite.depth_base + delta_depth, 1.0);
            out.tex_coords = vec2<f32>(sprite.top_left_tex_coords_x + sprite.width + anim_x_offset, sprite.top_left_tex_coords_y + sprite.height);
        }
        case 5u:{
            out.position = vec4<f32>(scale * gridify(sprite.top_left_position_x + sprite.width), gridify(sprite.top_left_position_y), sprite.depth_base + delta_depth, 1.0);
            out.tex_coords = vec2<f32>(sprite.top_left_tex_coords_x + sprite.width + anim_x_offset, sprite.top_left_tex_coords_y);
        }
        default:{
            out.position = vec4<f32>(0.0, 0.0, 0.0, 1.0);
            out.tex_coords = vec2<f32>(0.0, 0.0);
        }
    }
    return out;
}

    @group(0) @binding(0) var my_texture: texture_2d<f32>;

    @fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var result = textureLoad(my_texture, vec2<i32>(in.tex_coords), 0);
    // result.y = abs(sin(uniform_data.delta_time * 10000.0));
    return result;
}
