pub fn exp_f32(x: f32) -> f32 {
    let x = if x < -88.0 { -88.0 } else if x > 88.0 { 88.0 } else { x };
    let a = (12102203.0f32 * x + 1065353216.0) as i32;
    if a < 0 {
        return 0.0;
    }
    f32::from_bits(a as u32)
}

pub fn sqrt_f32(x: f32) -> f32 {
    if x <= 0.0 {
        return 0.0;
    }
    let i = f32::to_bits(x);
    let i = 0x5f3759df - (i >> 1);
    let mut guess = 1.0 / f32::from_bits(i);
    guess = 0.5 * (guess + x / guess);
    guess = 0.5 * (guess + x / guess);
    guess = 0.5 * (guess + x / guess);
    guess
}

pub fn softmax(x: &mut [f32]) {
    if x.is_empty() {
        return;
    }
    let mut max = x[0];
    for &v in x.iter() {
        if v > max {
            max = v;
        }
    }
    let mut sum = 0.0f32;
    for v in x.iter_mut() {
        *v = exp_f32(*v - max);
        sum += *v;
    }
    if sum > 0.0 {
        let inv = 1.0 / sum;
        for v in x.iter_mut() {
            *v *= inv;
        }
    }
}

pub fn rmsnorm(out: &mut [f32], x: &[f32], weight: &[f32], eps: f32) {
    let n = x.len();
    let mut ss = 0.0f32;
    for i in 0..n {
        ss += x[i] * x[i];
    }
    ss = 1.0 / sqrt_f32(ss / n as f32 + eps);
    for i in 0..n {
        out[i] = weight[i] * (ss * x[i]);
    }
}

pub fn silu(x: f32) -> f32 {
    x / (1.0 + exp_f32(-x))
}

pub fn rope(q: &mut [f32], k: &mut [f32], dim: usize, pos: u32, theta: f32) {
    let half = dim / 2;
    for i in 0..half {
        let freq = 1.0 / pow_f32(theta, (2 * i) as f32 / dim as f32);
        let angle = pos as f32 * freq;
        let cos = cos_f32(angle);
        let sin = sin_f32(angle);

        let q0 = q[2 * i];
        let q1 = q[2 * i + 1];
        q[2 * i] = q0 * cos - q1 * sin;
        q[2 * i + 1] = q0 * sin + q1 * cos;

        let k0 = k[2 * i];
        let k1 = k[2 * i + 1];
        k[2 * i] = k0 * cos - k1 * sin;
        k[2 * i + 1] = k0 * sin + k1 * cos;
    }
}

fn pow_f32(base: f32, exp: f32) -> f32 {
    exp_f32(exp * ln_f32(base))
}

fn ln_f32(x: f32) -> f32 {
    if x <= 0.0 {
        return -88.0;
    }
    let bits = f32::to_bits(x);
    let exponent = ((bits >> 23) & 0xFF) as i32 - 127;
    let mantissa_bits = (bits & 0x007FFFFF) | 0x3F800000;
    let m = f32::from_bits(mantissa_bits);
    let m = m - 1.0;
    let ln_m = m * (1.0 - m * (0.5 - m * (1.0 / 3.0 - m * 0.25)));
    exponent as f32 * 0.6931472 + ln_m
}

fn cos_f32(x: f32) -> f32 {
    sin_f32(x + 1.5707963)
}

fn sin_f32(mut x: f32) -> f32 {
    const TWO_PI: f32 = 6.2831853;
    const PI: f32 = 3.1415927;
    x = x % TWO_PI;
    if x < 0.0 {
        x += TWO_PI;
    }
    if x > PI {
        x -= TWO_PI;
    }
    let x2 = x * x;
    let x3 = x2 * x;
    let x5 = x3 * x2;
    let x7 = x5 * x2;
    x - x3 / 6.0 + x5 / 120.0 - x7 / 5040.0
}
