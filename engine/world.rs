use std::ops::Range;

use crate::rand;
use crate::utils::*;

pub const MAX_SIZE_X: u32 = 256;
pub const MAX_SIZE_Y: u32 = 256;
pub const MAX_SIZE_Z: u32 = 256;

pub struct World {
    sx: u32,
    sy: u32,
    sz: u32,
    /// A 2-dimensional array of length of every span list
    sizes: Vec<u32>,
    /// A 3-dimensional array of `(top, bot): (u32, u32)`
    spans: Vec<u32>,
    needs_upload: bool,
}

struct Array3D<T: Copy> {
    sx: usize,
    sy: usize,
    sz: usize,
    data: Vec<T>,
}

struct Array2D<T: Copy> {
    sx: usize,
    sy: usize,
    data: Vec<T>,
}

impl World {
    pub fn new(sx: usize, sy: usize, sz: usize) -> Self {
        let mut arr = Array3D::new(0, sx, sy, sz);

        arr.fill_test_data();

        Self::from_array(&arr)
    }

    fn from_array(arr: &Array3D<u32>) -> Self {
        let mut sizes = Array2D::new(0, arr.sx, arr.sz);
        let mut spans = Array3D::new(0, arr.sx, arr.sy, arr.sz);

        for x in 0..arr.sx {
            for z in 0..arr.sz {
                let mut start = if arr.get(x, 0, z) > 0 { Some(0) } else { None };

                for y in 1..arr.sy {
                    if arr.get(x, y, z) == 0 {
                        if let Some(bot) = start {
                            let len = sizes.as_mut(x, z);
                            let i = (*len) as usize;
                            let top = to_u32(y);

                            spans.set(x, i * 2, z, bot);
                            spans.set(x, i * 2 + 1, z, top);

                            *len += 1;

                            start = None;
                        }
                    } else if start.is_none() {
                        start = Some(to_u32(y));
                    }
                }
            }
        }

        Self {
            sx: to_u32(arr.sx),
            sy: to_u32(arr.sy),
            sz: to_u32(arr.sz),
            spans: spans.data,
            sizes: sizes.data,
            needs_upload: true,
        }
    }

    pub fn size_x(&self) -> u32 {
        self.sx
    }

    pub fn size_y(&self) -> u32 {
        self.sy
    }

    pub fn size_z(&self) -> u32 {
        self.sz
    }

    pub fn sizes(&self) -> &[u32] {
        &self.sizes
    }

    pub fn spans(&self) -> &[u32] {
        &self.spans
    }

    pub fn needs_upload(&self) -> bool {
        self.needs_upload
    }

    pub fn uploaded(&mut self) {
        self.needs_upload = false;
    }
}

impl<T: Copy> Array3D<T> {
    fn new(init: T, sx: usize, sy: usize, sz: usize) -> Self {
        let data = vec![init; sx * sy * sz];

        Self { sx, sy, sz, data }
    }

    fn get(&self, x: usize, y: usize, z: usize) -> T {
        self.assert_size_bounds(x, y, z);
        self.data[z * self.sy * self.sx + y * self.sx + x]
    }

    fn set(&mut self, x: usize, y: usize, z: usize, val: T) {
        self.assert_size_bounds(x, y, z);
        self.data[z * self.sy * self.sx + y * self.sx + x] = val;
    }

    fn as_mut(&mut self, x: usize, y: usize, z: usize) -> &mut T {
        self.assert_size_bounds(x, y, z);
        &mut self.data[z * self.sy * self.sx + y * self.sx + x]
    }

    fn assert_size_bounds(&self, x: usize, y: usize, z: usize) {
        assert!(x < self.sx, "x out of bounds: {} >= {}", x, self.sx);
        assert!(y < self.sy, "y out of bounds: {} >= {}", y, self.sy);
        assert!(z < self.sz, "z out of bounds: {} >= {}", z, self.sz);
    }
}

impl Array3D<u32> {
    fn fill_test_data(&mut self) {
        self.fill_with_spheres(0xcafe_babe, 50, 3..25);

        self.set(20, 20, self.sz / 2 - 3, 1);

        self.set(self.sx / 2, self.sy / 2, self.sz / 2 - 3, 1);
    }

    fn fill_with_spheres(&mut self, seed: u64, num_spheres: usize, sz_range: Range<u64>) {
        let mut rng = rand::Wyhash64::from_seed(seed);

        for _ in 0..num_spheres {
            let r = rng.gen_in_range(sz_range.clone());
            let x = rng.gen_in_range(0..self.sx as u64);
            let y = rng.gen_in_range(0..self.sy as u64);
            let z = rng.gen_in_range(0..self.sz as u64);

            self.add_sphere(x, y, z, r);
        }
    }

    fn add_sphere(&mut self, x: u64, y: u64, z: u64, radius: u64) {
        let x_min = x.saturating_sub(radius);
        let y_min = y.saturating_sub(radius);
        let z_min = z.saturating_sub(radius);

        let x_max = (x + radius).min(self.sx as u64);
        let y_max = (y + radius).min(self.sy as u64);
        let z_max = (z + radius).min(self.sz as u64);

        for dx in x_min..x_max {
            for dy in y_min..y_max {
                #[allow(clippy::cast_possible_wrap)]
                for dz in z_min..z_max {
                    let x_off = (dx as i64) - (x as i64);
                    let y_off = (dy as i64) - (y as i64);
                    let z_off = (dz as i64) - (z as i64);
                    let rad_sq = (radius as i64).pow(2);

                    if x_off * x_off + y_off * y_off + z_off * z_off <= rad_sq {
                        #[allow(clippy::cast_possible_truncation)]
                        self.set(dx as usize, dy as usize, dz as usize, 1);
                    }
                }
            }
        }
    }
}

impl<T: Copy> Array2D<T> {
    fn new(init: T, sx: usize, sy: usize) -> Self {
        let data = vec![init; sx * sy];

        Self { sx, sy, data }
    }

    fn as_mut(&mut self, x: usize, y: usize) -> &mut T {
        self.assert_size_bounds(x, y);
        &mut self.data[y * self.sx + x]
    }

    fn assert_size_bounds(&self, x: usize, y: usize) {
        assert!(x < self.sx, "x out of bounds: {} >= {}", x, self.sx);
        assert!(y < self.sy, "y out of bounds: {} >= {}", y, self.sy);
    }

    fn set(&mut self, x: usize, y: usize, val: T) {
        self.assert_size_bounds(x, y);
        self.data[y * self.sx + x] = val;
    }
}
