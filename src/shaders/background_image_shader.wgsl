struct VertexOutput {
  @builtin(position) clip_position: vec4<f32>,
  @location(0) uv: vec2<f32>,
};

@group(0) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(0) @binding(1)
var s_diffuse: sampler;

@group(0) @binding(2)
var<uniform> windowSize: vec2<f32>;

@group(0) @binding(3)
var<uniform> windowPos: vec2<f32>;

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
  let imageSize = vec2(1920.0, 1080.0);

  let quad = array<vec2<f32>, 4>(
    vec2(-1.0, -1.0),
    vec2( 1.0, -1.0),
    vec2(-1.0,  1.0),
    vec2( 1.0,  1.0),
  );

  let uvs = array<vec2<f32>, 4>(
    vec2(0.0, 0.0),
    vec2(1.0, 0.0),
    vec2(0.0, 1.0),
    vec2(1.0, 1.0),
  );

  let pos = quad[vertex_index];
  var uv = uvs[vertex_index];

  uv = uv * windowSize;
  uv = (windowPos + uv) / imageSize;

  var out: VertexOutput;
  out.clip_position = vec4(pos, 0.0, 1.0);
  out.uv = vec2(uv.x, 1.0 - uv.y);

  return out;
}


@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
  return textureSample(t_diffuse, s_diffuse, in.uv);
}
