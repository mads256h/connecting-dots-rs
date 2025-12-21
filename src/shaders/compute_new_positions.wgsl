struct Point {
  position: vec2<f32>,
  velocity: vec2<f32>,
}

struct Points {
  data: array<Point>,
};


@group(0) @binding(0) var<storage, read_write> points: Points;

@group(0) @binding(1) var<uniform> windowSize : vec2<f32>;

@group(0) @binding(2) var<uniform> deltaTime : f32;


@compute
@workgroup_size(64)
fn main(
  @builtin(global_invocation_id) id: vec3<u32>,
  ) {
  let i = id.x;

  let count = arrayLength(&points.data);

  if (i >= count) {
    return;
  }

  var p = points.data[i];

  p.position += p.velocity * deltaTime;

  if (p.position.x < 0.0 || p.position.x > windowSize.x) {
    p.velocity.x = -p.velocity.x;
  }

  if (p.position.y < 0.0 || p.position.y > windowSize.y) {
    p.velocity.y = -p.velocity.y;
  }

  p.position = clamp(p.position, vec2(0.0), windowSize);

  points.data[i] = p;
}
