struct VertexOutput {
  @builtin(position) clip_position: vec4<f32>,
  @location(0) vert_pos: vec2<f32>,
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

@group(0) @binding(2)
var<uniform> pointSize: f32;

@group(0) @binding(3)
var<uniform> intensity: f32;

@vertex
fn vs_main(
  @builtin(vertex_index) vertex_index: u32,
  @builtin(instance_index) instance_index: u32,
  ) -> VertexOutput {

  // Rendered using Triangle Strip:
  // 3  4
  // 
  // 1  2
  let quad = array<vec2<f32>, 4>(
    vec2(-1.0, -1.0),
    vec2( 1.0, -1.0),
    vec2(-1.0,  1.0),
    vec2( 1.0,  1.0),
  );

  var out: VertexOutput;

  let p = points.data[instance_index].position;

  let offset = quad[vertex_index] * pointSize * 0.5;
  let world = p + offset;

  let ndc = vec2(
    (world.x / windowSize.x) * 2.0 - 1.0,
    1.0 - (world.y / windowSize.y) * 2.0
  );


  out.clip_position = vec4<f32>(ndc, 0.0, 1.0);
  out.vert_pos = quad[vertex_index];
  return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
  let len = length(in.vert_pos);
  if (len > 1.0) {
    discard;
  }

  let falloff_point = 0.75;

  if (len > falloff_point) {
    let intens = min(intensity, (1.0 - (len - falloff_point)) * intensity);
    return vec4<f32>(1.0, 1.0, 1.0, intens);
  }


  return vec4<f32>(1.0, 1.0, 1.0, intensity);
}


// Idea:
// Compute shader calculates the new position of each point stored in another array
// Compute shader calculates the distance between each point and their higher index stored in an array of size [(POINTS * (POINTS - 1)) / 2]

// Vertex shader makes points and lines for POINTS and (POINTS * (POINTS - 1)) / 2

// Fragment shader discards if distance of line is greater than MIN_DISTANCE
