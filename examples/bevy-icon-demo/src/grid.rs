//! Pure math for placing icons on a fixed 3×3 grid in Bevy's centered,
//! y-up 2D coordinate space.

/// Number of columns/rows in the demo grid.
pub const COLUMNS: usize = 3;
pub const ROWS: usize = 3;

/// World-space (x, y) translation of the center of grid cell `index`
/// (row-major), given the square `cell_size` in pixels. The grid is centered
/// on the origin; +y is up.
pub fn cell_translation(index: usize, cell_size: f32) -> (f32, f32) {
    let column = (index % COLUMNS) as f32;
    let row = (index / COLUMNS) as f32;
    let x = (column - (COLUMNS as f32 - 1.0) / 2.0) * cell_size;
    let y = ((ROWS as f32 - 1.0) / 2.0 - row) * cell_size;
    (x, y)
}

#[cfg(test)]
mod tests {
    use super::cell_translation;

    #[test]
    fn center_cell_is_origin() {
        let (x, y) = cell_translation(4, 200.0);
        assert_eq!((x, y), (0.0, 0.0));
    }

    #[test]
    fn top_left_cell_is_up_and_left() {
        let (x, y) = cell_translation(0, 200.0);
        assert_eq!((x, y), (-200.0, 200.0));
    }

    #[test]
    fn bottom_right_cell_is_down_and_right() {
        let (x, y) = cell_translation(8, 200.0);
        assert_eq!((x, y), (200.0, -200.0));
    }
}
