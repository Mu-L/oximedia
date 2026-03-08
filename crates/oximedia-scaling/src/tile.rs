//! Image tiling for spatial indexing and parallel processing.

#![allow(dead_code)]

/// A rectangular tile within an image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tile {
    /// Left edge of the tile (inclusive)
    pub x: u32,
    /// Top edge of the tile (inclusive)
    pub y: u32,
    /// Width of the tile in pixels
    pub width: u32,
    /// Height of the tile in pixels
    pub height: u32,
}

impl Tile {
    /// Create a new tile.
    pub fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Return the area (width * height) of this tile.
    pub fn area(&self) -> u64 {
        self.width as u64 * self.height as u64
    }

    /// Return true if the pixel `(px, py)` is inside this tile.
    pub fn contains(&self, px: u32, py: u32) -> bool {
        px >= self.x
            && px < self.x.saturating_add(self.width)
            && py >= self.y
            && py < self.y.saturating_add(self.height)
    }

    /// Return true if this tile overlaps with `other`.
    pub fn overlaps(&self, other: &Tile) -> bool {
        let self_right = self.x.saturating_add(self.width);
        let self_bottom = self.y.saturating_add(self.height);
        let other_right = other.x.saturating_add(other.width);
        let other_bottom = other.y.saturating_add(other.height);

        self.x < other_right
            && self_right > other.x
            && self.y < other_bottom
            && self_bottom > other.y
    }
}

/// A regular grid of tiles covering an image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TileGrid {
    /// Number of tile columns
    pub cols: u32,
    /// Number of tile rows
    pub rows: u32,
    /// Width of each tile in pixels
    pub tile_w: u32,
    /// Height of each tile in pixels
    pub tile_h: u32,
}

impl TileGrid {
    /// Create a `TileGrid` that covers an image of `img_w x img_h` pixels
    /// using tiles of size `tile_size x tile_size`.
    ///
    /// The number of columns and rows is rounded up so the grid covers
    /// the full image.
    pub fn new(img_w: u32, img_h: u32, tile_size: u32) -> Self {
        let tile_size = tile_size.max(1);
        let cols = img_w.div_ceil(tile_size);
        let rows = img_h.div_ceil(tile_size);
        Self {
            cols,
            rows,
            tile_w: tile_size,
            tile_h: tile_size,
        }
    }

    /// Return the total number of tiles in the grid.
    pub fn tile_count(&self) -> u32 {
        self.cols * self.rows
    }

    /// Return the `Tile` at grid position `(col, row)`.
    ///
    /// The tile's position is `(col * tile_w, row * tile_h)`.
    pub fn tile_at(&self, col: u32, row: u32) -> Tile {
        Tile {
            x: col * self.tile_w,
            y: row * self.tile_h,
            width: self.tile_w,
            height: self.tile_h,
        }
    }

    /// Return the tile that contains pixel `(px, py)`, or `None` if out of range.
    pub fn tile_for_pixel(&self, px: u32, py: u32) -> Option<Tile> {
        let col = px / self.tile_w;
        let row = py / self.tile_h;
        if col < self.cols && row < self.rows {
            Some(self.tile_at(col, row))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tile_new() {
        let t = Tile::new(10, 20, 64, 64);
        assert_eq!(t.x, 10);
        assert_eq!(t.y, 20);
        assert_eq!(t.width, 64);
        assert_eq!(t.height, 64);
    }

    #[test]
    fn test_tile_area() {
        let t = Tile::new(0, 0, 100, 200);
        assert_eq!(t.area(), 20_000);
    }

    #[test]
    fn test_tile_area_large() {
        // Ensure no overflow with u32 dimensions
        let t = Tile::new(0, 0, 65535, 65535);
        assert_eq!(t.area(), 65535u64 * 65535u64);
    }

    #[test]
    fn test_tile_contains_inside() {
        let t = Tile::new(10, 10, 20, 20);
        assert!(t.contains(15, 15));
        assert!(t.contains(10, 10));
    }

    #[test]
    fn test_tile_contains_outside() {
        let t = Tile::new(10, 10, 20, 20);
        assert!(!t.contains(30, 15)); // right edge exclusive
        assert!(!t.contains(5, 15)); // left of tile
        assert!(!t.contains(15, 5)); // above tile
    }

    #[test]
    fn test_tile_contains_bottom_right() {
        let t = Tile::new(0, 0, 8, 8);
        assert!(t.contains(7, 7));
        assert!(!t.contains(8, 7));
        assert!(!t.contains(7, 8));
    }

    #[test]
    fn test_tile_overlaps_true() {
        let a = Tile::new(0, 0, 10, 10);
        let b = Tile::new(5, 5, 10, 10);
        assert!(a.overlaps(&b));
        assert!(b.overlaps(&a));
    }

    #[test]
    fn test_tile_overlaps_adjacent_no_overlap() {
        let a = Tile::new(0, 0, 10, 10);
        let b = Tile::new(10, 0, 10, 10); // right next to a, no overlap
        assert!(!a.overlaps(&b));
    }

    #[test]
    fn test_tile_overlaps_self() {
        let t = Tile::new(5, 5, 10, 10);
        assert!(t.overlaps(&t));
    }

    #[test]
    fn test_tile_overlaps_disjoint() {
        let a = Tile::new(0, 0, 5, 5);
        let b = Tile::new(100, 100, 5, 5);
        assert!(!a.overlaps(&b));
    }

    #[test]
    fn test_grid_new() {
        let g = TileGrid::new(100, 100, 32);
        assert_eq!(g.cols, 4); // ceil(100/32) = 4
        assert_eq!(g.rows, 4);
        assert_eq!(g.tile_w, 32);
        assert_eq!(g.tile_h, 32);
    }

    #[test]
    fn test_grid_tile_count() {
        let g = TileGrid::new(64, 64, 16);
        assert_eq!(g.tile_count(), 16); // 4*4
    }

    #[test]
    fn test_grid_tile_count_non_divisible() {
        let g = TileGrid::new(100, 100, 32);
        // ceil(100/32) = 4, so 4*4 = 16
        assert_eq!(g.tile_count(), 16);
    }

    #[test]
    fn test_grid_tile_at() {
        let g = TileGrid::new(256, 256, 64);
        let t = g.tile_at(1, 2);
        assert_eq!(t.x, 64);
        assert_eq!(t.y, 128);
        assert_eq!(t.width, 64);
        assert_eq!(t.height, 64);
    }

    #[test]
    fn test_grid_tile_for_pixel_found() {
        let g = TileGrid::new(256, 256, 64);
        let t = g.tile_for_pixel(70, 130);
        assert!(t.is_some());
        let t = t.expect("should succeed in test");
        // pixel (70,130) -> col=1, row=2 -> tile at (64, 128)
        assert_eq!(t.x, 64);
        assert_eq!(t.y, 128);
    }

    #[test]
    fn test_grid_tile_for_pixel_out_of_range() {
        let g = TileGrid::new(100, 100, 32);
        // Grid has 4 cols (0..=3) covering x 0..128, but image is 100 wide.
        // Pixel at (200, 50) is beyond cols
        let t = g.tile_for_pixel(200, 50);
        assert!(t.is_none());
    }

    #[test]
    fn test_grid_zero_size_image() {
        let g = TileGrid::new(0, 0, 32);
        assert_eq!(g.tile_count(), 0);
    }
}
