fn main() -> ui::Result {
	let mut plot = ui::Plot::new("PQ10 inverse EOTF", vector::xy{x: "linear", y: "PQ10"}, ["inverse EOTF".into()].into());
	let count = 0x4000;
	plot.x_values = (0..count).map(|index| (index as f64)/(count-1) as f64).collect();
	plot.sets = [plot.x_values.iter().map(|&linear| image::PQ10(linear as f32) as f64).collect()].into();
	 ui::run(&mut plot)
}
