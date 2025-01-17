use std::ops::Range;

use super::{color::Color, curves};
use crate::compose;

use gtk4::glib;
use rand_distr::{Distribution, Uniform};
use serde::{Deserialize, Serialize};
use svg::node::element::{self, Element};

/// The distribution for the spread of dots across the width of the textured stroke
#[derive(Debug, Eq, PartialEq, Clone, Copy, glib::Enum, Serialize, Deserialize)]
#[repr(u32)]
#[enum_type(name = "TexturedDotsDistribution")]
pub enum TexturedDotsDistribution {
    #[enum_value(name = "Uniform", nick = "uniform")]
    Uniform = 0,
    #[enum_value(name = "Normal", nick = "normal")]
    Normal,
    #[enum_value(name = "Exponential", nick = "exponential")]
    Exponential,
    #[enum_value(name = "ReverseExponential", nick = "reverse-exponential")]
    ReverseExponential,
}

impl Default for TexturedDotsDistribution {
    fn default() -> Self {
        Self::Normal
    }
}

impl TexturedDotsDistribution {
    /// Samples a value for the given range, symmetrical to the mid of the range. For distributions that are open ended, samples are clipped to the range
    fn sample_for_range_symmetrical_clipped<G: rand::Rng + ?Sized>(
        &self,
        rng: &mut G,
        range: Range<f64>,
    ) -> f64 {
        let sample = match self {
            Self::Uniform => rand_distr::Uniform::from(range.clone()).sample(rng),
            Self::Normal => {
                // setting the mean to the mid of the range
                let mean = (range.end + range.start) / 2.0;
                // the standard deviation
                let std_dev = ((range.end - range.start) / 2.0) / 3.0;

                rand_distr::Normal::new(mean, std_dev).unwrap().sample(rng)
            }
            Self::Exponential => {
                let mid = (range.end + range.start) / 2.0;
                let width = (range.end - range.start) / 4.0;
                // The lambda
                let lambda = 1.0;

                let sign: f64 = if rand_distr::Standard.sample(rng) {
                    1.0
                } else {
                    -1.0
                };

                mid + sign * width * rand_distr::Exp::new(lambda).unwrap().sample(rng)
            }
            Self::ReverseExponential => {
                let width = (range.end - range.start) / 4.0;
                // The lambda
                let lambda = 1.0;

                let positive: bool = rand_distr::Standard.sample(rng);
                let sign = if positive { 1.0 } else { -1.0 };
                let offset = if positive { range.start } else { range.end };

                offset + (sign * width * rand_distr::Exp::new(lambda).unwrap().sample(rng))
            }
        };

        if !range.contains(&sample) {
            // Do a uniform distribution as fallback if sample is out of range
            rand_distr::Uniform::from(range.clone()).sample(rng)
        } else {
            sample
        }
    }
}

/// The Options of how a textured shape should look

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default, rename = "textured_options")]
pub struct TexturedOptions {
    /// An optional seed to generate reproducable strokes
    #[serde(rename = "seed")]
    pub seed: Option<u64>,
    /// The width
    #[serde(rename = "width")]
    pub width: f64,
    /// The color of the stroke
    #[serde(rename = "stroke_color")]
    pub stroke_color: Option<Color>,
    /// Amount dots per 10x10 area
    #[serde(rename = "density")]
    pub density: f64,
    /// the radii of the dots
    #[serde(rename = "radii")]
    pub radii: na::Vector2<f64>,
    /// the distribution type
    #[serde(rename = "distribution")]
    pub distribution: TexturedDotsDistribution,
}

impl Default for TexturedOptions {
    fn default() -> Self {
        Self {
            seed: None,
            width: Self::WIDTH_DEFAULT,
            density: Self::DENSITY_DEFAULT,
            stroke_color: Some(Self::COLOR_DEFAULT),
            radii: Self::RADII_DEFAULT,
            distribution: TexturedDotsDistribution::default(),
        }
    }
}

impl TexturedOptions {
    /// The default width
    pub const WIDTH_DEFAULT: f64 = 1.0;
    /// The default color
    pub const COLOR_DEFAULT: Color = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    /// Density default
    pub const DENSITY_DEFAULT: f64 = 5.0;
    /// Radii default
    pub const RADII_DEFAULT: na::Vector2<f64> = na::vector![2.0, 0.3];
}

pub fn compose_line(line: curves::Line, width: f64, options: &TexturedOptions) -> Element {
    let mut rng = compose::new_rng_default_pcg64(options.seed);

    let rect = line.line_w_width_to_rect(width);
    let area = 4.0 * rect.cuboid.half_extents[0] * rect.cuboid.half_extents[1];

    // Ranges for randomization
    let range_x = -rect.cuboid.half_extents[0]..rect.cuboid.half_extents[0];
    let range_y = -rect.cuboid.half_extents[1]..rect.cuboid.half_extents[1];
    let range_dots_rot = -std::f64::consts::FRAC_PI_8..std::f64::consts::FRAC_PI_8;
    let range_dots_rx = options.radii[0] * 0.8..options.radii[0] * 1.25;
    let range_dots_ry = options.radii[1] * 0.8..options.radii[1] * 1.25;

    let distr_x = Uniform::from(range_x);
    let distr_dots_rot = Uniform::from(range_dots_rot);
    let distr_dots_rx = Uniform::from(range_dots_rx);
    let distr_dots_ry = Uniform::from(range_dots_ry);

    let n_dots = (area * 0.1 * options.density).round() as i32;
    let vec = line.end - line.start;

    let mut group = element::Group::new();

    for _ in 0..n_dots {
        let x_pos = distr_x.sample(&mut rng);
        let y_pos = options
            .distribution
            .sample_for_range_symmetrical_clipped(&mut rng, range_y.clone());

        let pos = rect.transform.transform * na::point![x_pos, y_pos];

        let rotation_angle = na::Rotation2::rotation_between(&na::Vector2::x(), &vec).angle()
            + distr_dots_rot.sample(&mut rng);
        let radii = na::vector![
            distr_dots_rx.sample(&mut rng),
            distr_dots_ry.sample(&mut rng)
        ];

        let fill = options
            .stroke_color
            .map_or(String::from(""), |color| color.to_css_color());

        let ellipse = element::Ellipse::new()
            .set(
                "transform",
                format!(
                    "rotate({},{},{})",
                    rotation_angle.to_degrees(),
                    pos[0],
                    pos[1]
                ),
            )
            .set("cx", pos[0])
            .set("cy", pos[1])
            .set("rx", radii[0])
            .set("ry", radii[1])
            .set("fill", fill);

        group = group.add(ellipse);
    }

    group.into()
}
