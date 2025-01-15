struct VertexOutput {
	@builtin(position) position: vec4<f32>,
}

@vertex fn vertex(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
	let texture_coordinates = vec2(f32(vertex_index >> 1), f32(vertex_index & 1)) * 2.;
	return VertexOutput(vec4(texture_coordinates * vec2(2., -2.) + vec2(-1., 1.), 0., 1.));
}

@fragment fn fragment(vertex: VertexOutput) -> @location(0) vec4<f32> {
	return vec4(1.0, 0.0, 0.0, 1.0);
}
