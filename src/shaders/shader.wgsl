struct VertexOutput {
  @builtin(position) clip_position: vec4<f32>,
  @location(0) vert_pos: vec3<f32>,
};

struct Point {
  position: vec2<f32>,
  velocity: vec2<f32>,
};

struct Points {
  data: array<Point>,
}

@group(0) @binding(0)
var<storage, read> points: Points;

@group(0) @binding(1)
var<uniform> windowSize: vec2<f32>;

@vertex
fn vs_main(
  @builtin(vertex_index) in_vertex_index: u32,
  ) -> VertexOutput {
  var out: VertexOutput;

  let p = points.data[in_vertex_index].position;

  let vertex_pos = (p / windowSize) * 2.0 - 1.0;

  out.clip_position = vec4<f32>(vertex_pos, 0.0, 1.0);
  out.vert_pos = out.clip_position.xyz;
  return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
  return vec4<f32>(1.0, 1.0, 1.0, 1.0);
}


// Idea:
// Compute shader calculates the new position of each point stored in another array
// Compute shader calculates the distance between each point and their higher index stored in an array of size [(POINTS * (POINTS - 1)) / 2]

// Vertex shader makes points and lines for POINTS and (POINTS * (POINTS - 1)) / 2

// Fragment shader discards if distance of line is greater than MIN_DISTANCE
