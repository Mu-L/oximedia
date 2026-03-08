//! Comprehensive particle system for VFX effects (fire, smoke, sparkles, etc.)
//!
//! Provides a high-level particle simulation with configurable emitter shapes,
//! physics, color interpolation, and a CPU rasterizer for rendering particles
//! to RGBA frame buffers.

// ── Vec2 ───────────────────────────────────────────────────────────────────────

/// 2D point with float coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec2 {
    /// X coordinate.
    pub x: f32,
    /// Y coordinate.
    pub y: f32,
}

impl Vec2 {
    /// Create a new vector.
    #[must_use]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Zero vector.
    #[must_use]
    pub const fn zero() -> Self {
        Self::new(0.0, 0.0)
    }

    /// Euclidean length.
    #[must_use]
    pub fn length(&self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    /// Return a unit-length vector in the same direction.
    /// Returns the zero vector if length is zero.
    #[must_use]
    pub fn normalize(&self) -> Self {
        let len = self.length();
        if len > 1e-10 {
            Self::new(self.x / len, self.y / len)
        } else {
            Self::zero()
        }
    }

    /// Dot product.
    #[must_use]
    pub fn dot(&self, other: Self) -> f32 {
        self.x * other.x + self.y * other.y
    }

    /// Component-wise addition.
    #[must_use]
    pub fn add(&self, other: Self) -> Self {
        Self::new(self.x + other.x, self.y + other.y)
    }

    /// Scale by a scalar factor.
    #[must_use]
    pub fn scale(&self, factor: f32) -> Self {
        Self::new(self.x * factor, self.y * factor)
    }

    /// Euclidean distance to another point.
    #[must_use]
    pub fn distance(&self, other: Self) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }
}

// ── Color ──────────────────────────────────────────────────────────────────────

/// RGBA color with float channels in [0.0, 1.0].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    /// Red component [0.0, 1.0].
    pub r: f32,
    /// Green component [0.0, 1.0].
    pub g: f32,
    /// Blue component [0.0, 1.0].
    pub b: f32,
    /// Alpha component [0.0, 1.0].
    pub a: f32,
}

impl Color {
    /// Create a new color from float components.
    #[must_use]
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Convert to RGBA u8 tuple.
    #[must_use]
    pub fn rgba_u8(&self) -> (u8, u8, u8, u8) {
        (
            (self.r.clamp(0.0, 1.0) * 255.0) as u8,
            (self.g.clamp(0.0, 1.0) * 255.0) as u8,
            (self.b.clamp(0.0, 1.0) * 255.0) as u8,
            (self.a.clamp(0.0, 1.0) * 255.0) as u8,
        )
    }

    /// Linearly interpolate towards `other` by factor `t` ∈ [0, 1].
    #[must_use]
    pub fn lerp(&self, other: Color, t: f32) -> Color {
        let t = t.clamp(0.0, 1.0);
        Color {
            r: self.r + (other.r - self.r) * t,
            g: self.g + (other.g - self.g) * t,
            b: self.b + (other.b - self.b) * t,
            a: self.a + (other.a - self.a) * t,
        }
    }

    /// Opaque white.
    #[must_use]
    pub const fn white() -> Self {
        Self::new(1.0, 1.0, 1.0, 1.0)
    }

    /// Opaque black.
    #[must_use]
    pub const fn black() -> Self {
        Self::new(0.0, 0.0, 0.0, 1.0)
    }

    /// Fully transparent black.
    #[must_use]
    pub const fn transparent() -> Self {
        Self::new(0.0, 0.0, 0.0, 0.0)
    }

    /// Orange-red fire color.
    #[must_use]
    pub const fn fire() -> Self {
        Self::new(1.0, 0.4, 0.0, 1.0)
    }

    /// Gray smoke color with partial transparency.
    #[must_use]
    pub const fn smoke() -> Self {
        Self::new(0.5, 0.5, 0.5, 0.5)
    }

    /// Bright yellow-white sparkle color.
    #[must_use]
    pub const fn sparkle() -> Self {
        Self::new(1.0, 0.98, 0.8, 1.0)
    }
}

// ── Particle ───────────────────────────────────────────────────────────────────

/// A single particle with full physics and visual state.
#[derive(Debug, Clone)]
pub struct Particle {
    /// Current position.
    pub position: Vec2,
    /// Current velocity.
    pub velocity: Vec2,
    /// Extra acceleration (beyond gravity).
    pub acceleration: Vec2,
    /// Current color (interpolated between start and end).
    pub color: Color,
    /// Color at birth.
    pub start_color: Color,
    /// Color at end of life.
    pub end_color: Color,
    /// Current size (pixels).
    pub size: f32,
    /// Size at birth.
    pub start_size: f32,
    /// Size at end of life.
    pub end_size: f32,
    /// Total lifetime in seconds.
    pub lifetime_s: f32,
    /// Elapsed age in seconds.
    pub age_s: f32,
    /// Rotation in radians.
    pub rotation: f32,
    /// Angular velocity in radians per second.
    pub angular_velocity: f32,
    /// Mass (affects gravity).
    pub mass: f32,
    /// Whether the particle is still alive.
    pub alive: bool,
}

impl Particle {
    /// Create a new particle with position, initial velocity and lifetime.
    #[must_use]
    pub fn new(pos: Vec2, vel: Vec2, lifetime_s: f32) -> Self {
        Self {
            position: pos,
            velocity: vel,
            acceleration: Vec2::zero(),
            color: Color::white(),
            start_color: Color::white(),
            end_color: Color::transparent(),
            size: 4.0,
            start_size: 4.0,
            end_size: 0.0,
            lifetime_s,
            age_s: 0.0,
            rotation: 0.0,
            angular_velocity: 0.0,
            mass: 1.0,
            alive: true,
        }
    }

    /// Life fraction: 0.0 = just born, 1.0 = dead.
    #[must_use]
    pub fn life_fraction(&self) -> f32 {
        if self.lifetime_s <= 0.0 {
            return 1.0;
        }
        (self.age_s / self.lifetime_s).clamp(0.0, 1.0)
    }

    /// Advance particle physics by `dt_s` seconds under `gravity`.
    pub fn update(&mut self, dt_s: f32, gravity: Vec2) {
        if !self.alive {
            return;
        }

        // Physics integration
        let total_accel = self.acceleration.add(gravity.scale(self.mass));
        self.velocity = self.velocity.add(total_accel.scale(dt_s));
        self.position = self.position.add(self.velocity.scale(dt_s));
        self.rotation += self.angular_velocity * dt_s;

        // Age
        self.age_s += dt_s;

        // Interpolate visual properties
        let t = self.life_fraction();
        self.color = self.start_color.lerp(self.end_color, t);
        self.size = self.start_size + (self.end_size - self.start_size) * t;
        self.size = self.size.max(0.0);

        // Death check
        if self.age_s >= self.lifetime_s {
            self.alive = false;
        }
    }

    /// Whether the particle is still alive.
    #[must_use]
    pub fn is_alive(&self) -> bool {
        self.alive
    }
}

// ── EmitterShape ───────────────────────────────────────────────────────────────

/// Emitter spawn shape.
#[derive(Debug, Clone, Copy)]
pub enum EmitterShape {
    /// Emit from a single point.
    Point {
        /// Spawn position.
        pos: Vec2,
    },
    /// Emit from within a circle.
    Circle {
        /// Circle center.
        center: Vec2,
        /// Circle radius.
        radius: f32,
    },
    /// Emit from within a rectangle.
    Rectangle {
        /// Left edge X.
        x: f32,
        /// Top edge Y.
        y: f32,
        /// Rectangle width.
        width: f32,
        /// Rectangle height.
        height: f32,
    },
    /// Emit along a line segment.
    Line {
        /// Line start.
        start: Vec2,
        /// Line end.
        end: Vec2,
    },
}

impl EmitterShape {
    /// Sample a spawn position using a deterministic LCG based on `seed`.
    #[must_use]
    pub fn sample_position(&self, seed: u64) -> Vec2 {
        // Simple LCG for deterministic but varied positions without requiring rand crate here.
        let a: u64 = 6_364_136_223_846_793_005;
        let c: u64 = 1_442_695_040_888_963_407;
        let s0 = seed.wrapping_mul(a).wrapping_add(c);
        let s1 = s0.wrapping_mul(a).wrapping_add(c);

        // Map to [0, 1)
        let t0 = (s0 >> 33) as f32 / (1u64 << 31) as f32;
        let t1 = (s1 >> 33) as f32 / (1u64 << 31) as f32;

        match *self {
            EmitterShape::Point { pos } => pos,
            EmitterShape::Circle { center, radius } => {
                // Uniform disk via rejection method approximation using angle+sqrt
                let angle = t0 * std::f32::consts::TAU;
                let r = t1.sqrt() * radius;
                Vec2::new(center.x + angle.cos() * r, center.y + angle.sin() * r)
            }
            EmitterShape::Rectangle {
                x,
                y,
                width,
                height,
            } => Vec2::new(x + t0 * width, y + t1 * height),
            EmitterShape::Line { start, end } => {
                let dx = end.x - start.x;
                let dy = end.y - start.y;
                Vec2::new(start.x + t0 * dx, start.y + t0 * dy)
            }
        }
    }
}

// ── EmitterConfig ──────────────────────────────────────────────────────────────

/// Configuration for a [`ParticleEmitter`].
#[derive(Debug, Clone)]
pub struct EmitterConfig {
    /// Emitter spawn shape.
    pub shape: EmitterShape,
    /// Particles emitted per second.
    pub emission_rate: f32,
    /// Optional burst: emit this many at once instead of streaming.
    pub burst_count: Option<u32>,
    /// Base particle lifetime in seconds.
    pub particle_lifetime_s: f32,
    /// Random variance added/subtracted from lifetime.
    pub particle_lifetime_variance_s: f32,
    /// Base initial velocity.
    pub initial_velocity: Vec2,
    /// Variance in velocity (±).
    pub velocity_variance: Vec2,
    /// Particle size at birth.
    pub initial_size: f32,
    /// Particle size at end of life.
    pub end_size: f32,
    /// Color at birth.
    pub start_color: Color,
    /// Color at end of life.
    pub end_color: Color,
    /// Gravity vector applied to all particles.
    pub gravity: Vec2,
    /// Maximum number of live particles.
    pub max_particles: usize,
}

impl EmitterConfig {
    /// Fire preset emitting from a point.
    #[must_use]
    pub fn fire(pos: Vec2) -> Self {
        Self {
            shape: EmitterShape::Point { pos },
            emission_rate: 60.0,
            burst_count: None,
            particle_lifetime_s: 1.2,
            particle_lifetime_variance_s: 0.4,
            initial_velocity: Vec2::new(0.0, -80.0),
            velocity_variance: Vec2::new(30.0, 20.0),
            initial_size: 8.0,
            end_size: 0.0,
            start_color: Color::fire(),
            end_color: Color::new(0.3, 0.0, 0.0, 0.0),
            gravity: Vec2::new(0.0, -20.0),
            max_particles: 512,
        }
    }

    /// Smoke preset emitting from a point.
    #[must_use]
    pub fn smoke(pos: Vec2) -> Self {
        Self {
            shape: EmitterShape::Point { pos },
            emission_rate: 15.0,
            burst_count: None,
            particle_lifetime_s: 3.0,
            particle_lifetime_variance_s: 1.0,
            initial_velocity: Vec2::new(0.0, -30.0),
            velocity_variance: Vec2::new(10.0, 5.0),
            initial_size: 10.0,
            end_size: 30.0,
            start_color: Color::smoke(),
            end_color: Color::new(0.5, 0.5, 0.5, 0.0),
            gravity: Vec2::new(0.0, -5.0),
            max_particles: 256,
        }
    }

    /// Sparkle preset emitting from a point.
    #[must_use]
    pub fn sparkle(pos: Vec2) -> Self {
        Self {
            shape: EmitterShape::Point { pos },
            emission_rate: 80.0,
            burst_count: None,
            particle_lifetime_s: 0.6,
            particle_lifetime_variance_s: 0.2,
            initial_velocity: Vec2::new(0.0, -120.0),
            velocity_variance: Vec2::new(60.0, 40.0),
            initial_size: 3.0,
            end_size: 0.0,
            start_color: Color::sparkle(),
            end_color: Color::new(1.0, 0.8, 0.2, 0.0),
            gravity: Vec2::new(0.0, 60.0),
            max_particles: 1024,
        }
    }

    /// Snowfall preset filling `area_width` pixels horizontally.
    #[must_use]
    pub fn snow(area_width: f32) -> Self {
        Self {
            shape: EmitterShape::Line {
                start: Vec2::new(0.0, 0.0),
                end: Vec2::new(area_width, 0.0),
            },
            emission_rate: 25.0,
            burst_count: None,
            particle_lifetime_s: 8.0,
            particle_lifetime_variance_s: 2.0,
            initial_velocity: Vec2::new(5.0, 40.0),
            velocity_variance: Vec2::new(15.0, 10.0),
            initial_size: 3.0,
            end_size: 2.0,
            start_color: Color::white(),
            end_color: Color::new(0.9, 0.9, 1.0, 0.0),
            gravity: Vec2::new(0.0, 10.0),
            max_particles: 400,
        }
    }
}

// ── ParticleEmitter ────────────────────────────────────────────────────────────

/// A configurable particle emitter that manages its own particle pool.
pub struct ParticleEmitter {
    /// Emitter configuration.
    pub config: EmitterConfig,
    particles: Vec<Particle>,
    /// Fractional accumulator for sub-frame emission.
    emission_accumulator: f32,
    /// Total particles ever emitted.
    total_emitted: u64,
    /// Elapsed simulation time in seconds.
    elapsed_s: f32,
}

impl ParticleEmitter {
    /// Create a new emitter with the given configuration.
    #[must_use]
    pub fn new(config: EmitterConfig) -> Self {
        let cap = config.max_particles.min(65536);
        Self {
            particles: Vec::with_capacity(cap),
            config,
            emission_accumulator: 0.0,
            total_emitted: 0,
            elapsed_s: 0.0,
        }
    }

    /// Advance simulation by `dt_s` seconds.
    ///
    /// Steps:
    /// 1. Update existing particles (physics + color/size lerp).
    /// 2. Remove dead particles.
    /// 3. Emit new particles according to `emission_rate`.
    pub fn update(&mut self, dt_s: f32) {
        let gravity = self.config.gravity;

        // 1 & 2: Update and cull
        for p in &mut self.particles {
            p.update(dt_s, gravity);
        }
        self.particles.retain(Particle::is_alive);

        // 3: Emit
        self.elapsed_s += dt_s;
        self.emission_accumulator += self.config.emission_rate * dt_s;
        let to_emit = self.emission_accumulator.floor() as u32;
        self.emission_accumulator -= to_emit as f32;
        self.emit_n(to_emit);
    }

    /// Number of currently live particles.
    #[must_use]
    pub fn live_count(&self) -> usize {
        self.particles.len()
    }

    /// Total number of particles ever emitted.
    #[must_use]
    pub fn total_emitted(&self) -> u64 {
        self.total_emitted
    }

    /// Borrow the particle slice.
    #[must_use]
    pub fn particles(&self) -> &[Particle] {
        &self.particles
    }

    /// Whether the emitter has reached its `max_particles` limit.
    #[must_use]
    pub fn is_full(&self) -> bool {
        self.particles.len() >= self.config.max_particles
    }

    /// Render all live particles to an RGBA frame as soft circles.
    ///
    /// Each particle is drawn as a radially soft (Gaussian-ish) circle blended
    /// over whatever is already in the buffer.
    pub fn render(&self, frame: &mut Vec<u8>, width: u32, height: u32) {
        let expected = (width as usize) * (height as usize) * 4;
        if frame.len() < expected {
            return;
        }

        for p in &self.particles {
            if !p.alive {
                continue;
            }
            let (pr, pg, pb, pa) = p.color.rgba_u8();
            let pa_f = pa as f32 / 255.0;
            let radius = p.size.max(0.5);
            let cx = p.position.x;
            let cy = p.position.y;

            let x_min = ((cx - radius).floor() as i32).max(0) as u32;
            let x_max = ((cx + radius).ceil() as i32).min(width as i32 - 1).max(0) as u32;
            let y_min = ((cy - radius).floor() as i32).max(0) as u32;
            let y_max = ((cy + radius).ceil() as i32).min(height as i32 - 1).max(0) as u32;

            for py in y_min..=y_max {
                for px in x_min..=x_max {
                    let dx = px as f32 - cx;
                    let dy = py as f32 - cy;
                    let dist = (dx * dx + dy * dy).sqrt();
                    if dist > radius {
                        continue;
                    }
                    // Soft falloff: 1 at center → 0 at edge
                    let falloff = (1.0 - dist / radius).clamp(0.0, 1.0);
                    let alpha = pa_f * falloff;

                    let idx = ((py * width + px) * 4) as usize;
                    let dst_r = frame[idx] as f32 / 255.0;
                    let dst_g = frame[idx + 1] as f32 / 255.0;
                    let dst_b = frame[idx + 2] as f32 / 255.0;
                    let dst_a = frame[idx + 3] as f32 / 255.0;

                    let src_r = pr as f32 / 255.0;
                    let src_g = pg as f32 / 255.0;
                    let src_b = pb as f32 / 255.0;

                    // Alpha over compositing
                    let out_a = alpha + dst_a * (1.0 - alpha);
                    if out_a > 1e-6 {
                        frame[idx] =
                            ((src_r * alpha + dst_r * dst_a * (1.0 - alpha)) / out_a * 255.0) as u8;
                        frame[idx + 1] =
                            ((src_g * alpha + dst_g * dst_a * (1.0 - alpha)) / out_a * 255.0) as u8;
                        frame[idx + 2] =
                            ((src_b * alpha + dst_b * dst_a * (1.0 - alpha)) / out_a * 255.0) as u8;
                        frame[idx + 3] = (out_a * 255.0) as u8;
                    }
                }
            }
        }
    }

    /// Immediately emit `count` particles (up to `max_particles`).
    pub fn burst(&mut self, count: u32) {
        self.emit_n(count);
    }

    // ── Internal ─────────────────────────────────────────────────────────────

    fn emit_n(&mut self, count: u32) {
        for _ in 0..count {
            if self.particles.len() >= self.config.max_particles {
                break;
            }
            let seed = self.total_emitted.wrapping_add(0xDEAD_BEEF_1234_5678);
            let p = self.make_particle(seed);
            self.particles.push(p);
            self.total_emitted += 1;
        }
    }

    fn make_particle(&self, seed: u64) -> Particle {
        let cfg = &self.config;
        let pos = cfg.shape.sample_position(seed);

        // Deterministic variance using a second seed derivative
        let a: u64 = 6_364_136_223_846_793_005;
        let c: u64 = 1_442_695_040_888_963_407;
        let sv0 = seed.wrapping_mul(a).wrapping_add(c);
        let sv1 = sv0.wrapping_mul(a).wrapping_add(c);
        let sv2 = sv1.wrapping_mul(a).wrapping_add(c);
        let sv3 = sv2.wrapping_mul(a).wrapping_add(c);

        // Map seeds to [-1, 1]
        let n0 = ((sv0 >> 32) as f32 / (u32::MAX as f32 / 2.0)) - 1.0;
        let n1 = ((sv1 >> 32) as f32 / (u32::MAX as f32 / 2.0)) - 1.0;
        let n2 = ((sv2 >> 32) as f32 / (u32::MAX as f32 / 2.0)) - 1.0;

        // Lifetime with variance
        let t_life = (sv3 >> 32) as f32 / u32::MAX as f32; // [0, 1]
        let lifetime = (cfg.particle_lifetime_s
            + (t_life * 2.0 - 1.0) * cfg.particle_lifetime_variance_s)
            .max(0.01);

        let vel = Vec2::new(
            cfg.initial_velocity.x + n0 * cfg.velocity_variance.x,
            cfg.initial_velocity.y + n1 * cfg.velocity_variance.y,
        );

        let mut p = Particle::new(pos, vel, lifetime);
        p.start_color = cfg.start_color;
        p.end_color = cfg.end_color;
        p.color = cfg.start_color;
        p.start_size = cfg.initial_size;
        p.end_size = cfg.end_size;
        p.size = cfg.initial_size;
        p.angular_velocity = n2 * std::f32::consts::PI;
        p
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vec2_length() {
        let v = Vec2::new(3.0, 4.0);
        assert!(
            (v.length() - 5.0).abs() < 1e-5,
            "length of (3,4) should be 5"
        );
    }

    #[test]
    fn test_vec2_normalize() {
        let v = Vec2::new(3.0, 4.0);
        let n = v.normalize();
        assert!(
            (n.length() - 1.0).abs() < 1e-5,
            "normalized vector should have length 1.0, got {}",
            n.length()
        );
    }

    #[test]
    fn test_vec2_dot() {
        let v1 = Vec2::new(1.0, 0.0);
        let v2 = Vec2::new(0.0, 1.0);
        assert!(
            (v1.dot(v2) - 0.0).abs() < 1e-5,
            "orthogonal vectors dot product should be 0"
        );
    }

    #[test]
    fn test_color_lerp() {
        let white = Color::white();
        let black = Color::black();
        let gray = white.lerp(black, 0.5);
        assert!(
            (gray.r - 0.5).abs() < 1e-5,
            "r should be 0.5, got {}",
            gray.r
        );
        assert!(
            (gray.g - 0.5).abs() < 1e-5,
            "g should be 0.5, got {}",
            gray.g
        );
        assert!(
            (gray.b - 0.5).abs() < 1e-5,
            "b should be 0.5, got {}",
            gray.b
        );
    }

    #[test]
    fn test_color_fire() {
        let fire = Color::fire();
        assert!(fire.r > 0.5, "fire.r should be > 0.5, got {}", fire.r);
        assert!(fire.g < 0.5, "fire.g should be < 0.5, got {}", fire.g);
    }

    #[test]
    fn test_particle_update_movement() {
        let pos = Vec2::new(100.0, 100.0);
        let vel = Vec2::new(10.0, 0.0);
        let mut p = Particle::new(pos, vel, 2.0);
        p.update(1.0, Vec2::zero());
        assert!(
            (p.position.x - 110.0).abs() < 1e-4,
            "x should be 110.0, got {}",
            p.position.x
        );
    }

    #[test]
    fn test_particle_dies_at_lifetime() {
        let mut p = Particle::new(Vec2::zero(), Vec2::zero(), 0.5);
        p.update(0.6, Vec2::zero());
        assert!(!p.alive, "particle should be dead after lifetime");
    }

    #[test]
    fn test_particle_life_fraction() {
        let p = Particle::new(Vec2::zero(), Vec2::zero(), 2.0);
        assert!((p.life_fraction() - 0.0).abs() < 1e-5, "just born → 0.0");

        let mut p2 = Particle::new(Vec2::zero(), Vec2::zero(), 2.0);
        p2.update(1.9, Vec2::zero());
        assert!(
            p2.life_fraction() > 0.9,
            "near end → close to 1.0, got {}",
            p2.life_fraction()
        );
    }

    #[test]
    fn test_emitter_fire_config() {
        let cfg = EmitterConfig::fire(Vec2::new(100.0, 100.0));
        assert!(cfg.emission_rate > 0.0, "fire emission_rate should be > 0");
    }

    #[test]
    fn test_emitter_update_emits_particles() {
        let cfg = EmitterConfig::fire(Vec2::new(50.0, 50.0));
        let mut emitter = ParticleEmitter::new(cfg);
        emitter.update(1.0);
        assert!(
            emitter.live_count() > 0,
            "should have live particles after 1s update"
        );
    }

    #[test]
    fn test_emitter_max_particles_respected() {
        let mut cfg = EmitterConfig::fire(Vec2::new(50.0, 50.0));
        cfg.max_particles = 10;
        cfg.emission_rate = 10_000.0;
        let mut emitter = ParticleEmitter::new(cfg);
        emitter.update(1.0);
        assert!(
            emitter.live_count() <= 10,
            "should not exceed max_particles, got {}",
            emitter.live_count()
        );
    }

    #[test]
    fn test_emitter_burst() {
        let mut cfg = EmitterConfig::fire(Vec2::new(50.0, 50.0));
        cfg.max_particles = 100;
        cfg.emission_rate = 0.0; // disable streaming
        let mut emitter = ParticleEmitter::new(cfg);
        let before = emitter.live_count();
        emitter.burst(20);
        assert!(
            emitter.live_count() > before,
            "burst should increase live_count, before={before} after={}",
            emitter.live_count()
        );
    }
}
