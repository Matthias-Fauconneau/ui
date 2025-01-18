struct VertexOutput {
	@builtin(position) position: vec4<f32>,
}

const triangle = array(vec2(-1., -1.), vec2(1., -1.), vec2(1., 1.));

@vertex fn vertex(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
	return VertexOutput(vec4(triangle[vertex_index], 0., 1.));
}

@fragment fn fragment(vertex: VertexOutput) -> @location(0) vec4<f32> {
	return vec4(1., 0., 0., 1.);
}
