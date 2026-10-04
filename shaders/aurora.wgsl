struct Orki {
  resolution : vec2<f32>,
  time       : f32,
  delta      : f32,
  mouse      : vec4<f32>,
  progress   : f32,
  phase      : f32,
  scale      : f32,
  dark       : f32,
  palette    : array<vec4<f32>, 8>,
  params     : array<vec4<f32>, 8>,
};

@group(0) @binding(0) var<uniform> orki : Orki;

@vertex
fn vs_main(@builtin(vertex_index) vi : u32) -> @builtin(position) vec4<f32> {
  var pos = array<vec2<f32>, 3>(
    vec2<f32>(-1.0, -1.0),
    vec2<f32>( 3.0, -1.0),
    vec2<f32>(-1.0,  3.0),
  );
  return vec4<f32>(pos[vi], 0.0, 1.0);
}

@fragment
fn fs_main(@builtin(position) frag_coord : vec4<f32>) -> @location(0) vec4<f32> {
  let uv = frag_coord.xy / orki.resolution;
  let t = orki.time * orki.params[0].x;
  let p = uv * (orki.resolution / orki.resolution.y);

  let band1 = 0.5 + 0.5 * sin(p.x * 1.6 + t) * cos(p.y * 1.1 - t * 0.6);
  let band2 = 0.5 + 0.5 * sin(p.x * 0.7 - t * 0.4 + p.y * 1.9);
  let glow = pow(band1 * band2, 1.6);

  let base = mix(orki.palette[0].rgb, orki.palette[1].rgb, band1);
  let col = mix(base, orki.palette[2].rgb, glow * orki.params[0].y);
  let vign = 1.0 - 0.35 * length(uv - vec2<f32>(0.5, 0.5));
  let final_col = mix(col, orki.palette[3].rgb, 1.0 - orki.progress * 0.35);

  return vec4(final_col * vign, 1.0);
}
