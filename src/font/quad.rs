use xy::vec2;

#[cfg(feature="kurbo")] 
pub fn quad(p0: vec2, p1: vec2, p2: vec2, mut line_to: impl FnMut(vec2)) {
	use kurbo::PathEl::*;
	fn point(vec2{x,y}: vec2) -> kurbo::Point { kurbo::Point{x: x as f64, y: y as f64} }
	let (p0,p1,p2) = (point(p0),point(p1),point(p2));
	kurbo::flatten([MoveTo(p0),QuadTo(p1,p2)], 1./4., |e| if let LineTo(kurbo::Point{x,y}) = e { line_to(vec2{x: x as f32, y: y as f32}); } /*Ignore first MoveTo*/)
}

#[cfg(not(feature="kurbo"))] 
pub fn quad(p0: vec2, p1: vec2, p2: vec2, mut line_to: impl FnMut(vec2)) {
    use crate::vector::{dot, cross, norm};

    // Modified from https://github.com/linebender/kurbo MIT

    /// An approximation to $\int (1 + 4x^2) ^ -1/4 dx$
    fn approx_parabola_integral(x: f32) -> f32 {
        const D: f32 = 0.67;
        x / (1. - D + (D.powi(4) + 1./4. * x * x).sqrt().sqrt())
    }

    /// An approximation to the inverse parabola integral.
    fn approx_parabola_inv_integral(x: f32) -> f32 {
        const B: f32 = 0.39;
        x * (1. - B + (B * B + 1./4. * x * x).sqrt())
    }

    struct Subdivision {
        a0: f32,
        a2: f32,
        u0: f32,
        uscale: f32,
        pub(crate) val: f32, // number of subdivisions * 2 * tolerance_sqrt
    }

    impl Subdivision {
        fn determine_subdiv_t(&self, t: f32) -> f32 {
            let a = self.a0 + (self.a2 - self.a0) * t;
            let u = approx_parabola_inv_integral(a);
            (u - self.u0) * self.uscale
        }
    }

    pub struct Quad {pub p0: vec2, pub p1: vec2, pub p2: vec2}

    impl Quad {
        fn eval(&self, t: f32) -> vec2 {
            let mt = 1. - t;
            t * (mt * mt) * self.p0 + ((mt * 2.) * self.p1 +  t * self.p2)
        }

        /// Estimate the number of subdivisions for flattening.
        fn estimate_subdiv(&self, tolerance_sqrt: f32) -> Subdivision {
            // Determine transformation to $y = x^2$ parabola.
            let d01 = self.p1 - self.p0;
            let d12 = self.p2 - self.p1;
            let dd = d01 - d12;
            let cross = cross(self.p2 - self.p0, dd);
            let x0 = dot(d01, dd) * cross.recip();
            let x2 = dot(d12, dd) * cross.recip();
            let scale = (cross / (norm(dd) * (x2 - x0))).abs();

            // Compute number of subdivisions needed.
            let a0 = approx_parabola_integral(x0);
            let a2 = approx_parabola_integral(x2);
            let val = if scale.is_finite() {
                let da = (a2 - a0).abs();
                let sqrt_scale = scale.sqrt();
                if x0.signum() == x2.signum() {
                    da * sqrt_scale
                } else {
                    // Handle cusp case (segment contains curvature maximum)
                    let xmin = tolerance_sqrt / sqrt_scale;
                    tolerance_sqrt * da / approx_parabola_integral(xmin)
                }
            } else {
                0.
            };
            let u0 = approx_parabola_inv_integral(a0);
            let u2 = approx_parabola_inv_integral(a2);
            let uscale = (u2 - u0).recip();
            Subdivision {
                a0,
                a2,
                u0,
                uscale,
                val,
            }
        }
    }

    let tolerance_sqrt = 1./2.;
    let subdivision = Quad{p0, p1, p2}.estimate_subdiv(tolerance_sqrt);
    let n = ((1./2. * subdivision.val / tolerance_sqrt).ceil() as usize).max(1);
    let step = 1. / (n as f32);
    for i in 1..(n - 1) {
        let u = (i as f32) * step;
        let t = subdivision.determine_subdiv_t(u);
        let p = Quad{p0, p1, p2}.eval(t);
        line_to(p);
    }
    line_to(p2);
}
