use bevy::prelude::*;

// Visual constants for shape definition
const X: bool = true;  // Occupied cell
const O: bool = false; // Empty cell

/// Tetromino-style shapes for inventory items
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TetrominoShape {
    Single,      // 1x1
    Horizontal2, // 2x1
    Vertical2,   // 1x2
    Square2x2,   // 2x2
    LPiece,      // L shape
    TPiece,      // 3x3 T shape (cross)
}

impl TetrominoShape {
    /// Get the shape grid for this tetromino
    pub fn grid(&self) -> &'static [&'static [bool]] {
        match self {
            TetrominoShape::Single => &[
                &[X],
            ],
            TetrominoShape::Horizontal2 => &[
                &[X, X],
            ],
            TetrominoShape::Vertical2 => &[
                &[X],
                &[X],
            ],
            TetrominoShape::Square2x2 => &[
                &[X, X],
                &[X, X],
            ],
            TetrominoShape::LPiece => &[
                &[X, O],
                &[X, O],
                &[X, X],
            ],
            TetrominoShape::TPiece => &[
                &[X, X, X],
                &[O, X, O],
                &[O, X, O],
            ],
        }
    }

    /// Get width of the shape grid
    pub fn width(&self) -> usize {
        self.grid()[0].len()
    }

    /// Get height of the shape grid
    pub fn height(&self) -> usize {
        self.grid().len()
    }

    /// Rotate the shape 90 degrees clockwise
    pub fn rotate_clockwise(&self) -> Vec<Vec<bool>> {
        let grid = self.grid();
        let height = grid.len();
        let width = grid[0].len();

        let mut rotated = vec![vec![false; height]; width];

        for y in 0..height {
            for x in 0..width {
                rotated[x][height - 1 - y] = grid[y][x];
            }
        }

        rotated
    }

    /// Get all occupied cell positions (relative to top-left)
    pub fn occupied_cells(&self) -> Vec<(usize, usize)> {
        let grid = self.grid();
        let mut cells = Vec::new();

        for (y, row) in grid.iter().enumerate() {
            for (x, &cell) in row.iter().enumerate() {
                if cell {
                    cells.push((x, y));
                }
            }
        }

        cells
    }
}

/// Represents an item that can be stored in inventory
#[derive(Component, Clone, Debug)]
pub struct Item {
    /// Display name of the item
    pub name: String,
    /// 2D texture for UI rendering (in inventory slots)
    pub texture_2d: Handle<Image>,
    /// Optional 3D model/texture for world rendering
    pub texture_3d: Option<Handle<Image>>,
    /// Shape of the item in the inventory grid
    pub shape: TetrominoShape,
    /// Current rotation state (0-3 for 90-degree increments)
    pub rotation: u8,
    /// Whether this item can be rotated
    pub rotation_locked: bool,
}

impl Item {
    pub fn new(name: impl Into<String>, texture_2d: Handle<Image>, shape: TetrominoShape) -> Self {
        Self {
            name: name.into(),
            texture_2d,
            texture_3d: None,
            shape,
            rotation: 0,
            rotation_locked: false,
        }
    }

    pub fn with_3d_texture(mut self, texture_3d: Handle<Image>) -> Self {
        self.texture_3d = Some(texture_3d);
        self
    }

    pub fn with_rotation_locked(mut self, locked: bool) -> Self {
        self.rotation_locked = locked;
        self
    }

    /// Get the current rotated grid for this item
    pub fn current_grid(&self) -> Vec<Vec<bool>> {
        if self.rotation == 0 {
            // No rotation, return base grid
            self.shape.grid().iter().map(|row| row.to_vec()).collect()
        } else {
            // Apply rotations
            let mut grid: Vec<Vec<bool>> = self.shape.grid().iter().map(|row| row.to_vec()).collect();
            for _ in 0..self.rotation {
                grid = Self::rotate_grid_clockwise(&grid);
            }
            grid
        }
    }

    /// Rotate a grid 90 degrees clockwise
    fn rotate_grid_clockwise(grid: &[Vec<bool>]) -> Vec<Vec<bool>> {
        let height = grid.len();
        let width = grid[0].len();
        let mut rotated = vec![vec![false; height]; width];

        for y in 0..height {
            for x in 0..width {
                rotated[x][height - 1 - y] = grid[y][x];
            }
        }

        rotated
    }

    /// Get width of current rotated state
    pub fn current_width(&self) -> usize {
        self.current_grid()[0].len()
    }

    /// Get height of current rotated state
    pub fn current_height(&self) -> usize {
        self.current_grid().len()
    }

    /// Get occupied cells in current rotation
    pub fn current_occupied_cells(&self) -> Vec<(usize, usize)> {
        let grid = self.current_grid();
        let mut cells = Vec::new();

        for (y, row) in grid.iter().enumerate() {
            for (x, &cell) in row.iter().enumerate() {
                if cell {
                    cells.push((x, y));
                }
            }
        }

        cells
    }

    /// Rotate the item clockwise
    pub fn rotate(&mut self) {
        if !self.rotation_locked {
            self.rotation = (self.rotation + 1) % 4;
        }
    }
}

/// Unique identifier for an item in the inventory
pub type ItemId = usize;

/// Information about a cell in the inventory grid
#[derive(Clone, Debug)]
pub struct InventoryCell {
    /// ID of the item occupying this cell (if any)
    pub item_id: Option<ItemId>,
    /// Whether this cell is the anchor (top-left) of the item
    pub is_anchor: bool,
}

impl Default for InventoryCell {
    fn default() -> Self {
        Self {
            item_id: None,
            is_anchor: false,
        }
    }
}

/// Player inventory component - stores items in a Tetris-style grid
#[derive(Component, Clone, Debug)]
pub struct Inventory {
    /// Grid of cells tracking what item occupies each space
    pub grid: Vec<Vec<InventoryCell>>,
    /// Items stored in inventory, indexed by ItemId
    pub items: Vec<Option<Item>>,
    /// Next available item ID
    next_id: ItemId,
    /// Width of the inventory grid
    pub width: usize,
    /// Height of the inventory grid
    pub height: usize,
}

impl Inventory {
    /// Create a new inventory with the specified dimensions
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            grid: vec![vec![InventoryCell::default(); width]; height],
            items: Vec::new(),
            next_id: 0,
            width,
            height,
        }
    }

    /// Check if a position is within grid bounds
    fn in_bounds(&self, x: usize, y: usize) -> bool {
        x < self.width && y < self.height
    }

    /// Check if an item can be placed at the given position
    pub fn can_place_item(&self, item: &Item, anchor_x: usize, anchor_y: usize) -> bool {
        let cells = item.current_occupied_cells();

        for (dx, dy) in cells {
            let x = anchor_x + dx;
            let y = anchor_y + dy;

            // Check bounds
            if !self.in_bounds(x, y) {
                return false;
            }

            // Check if cell is occupied
            if self.grid[y][x].item_id.is_some() {
                return false;
            }
        }

        true
    }

    /// Place an item at the given anchor position
    pub fn place_item(&mut self, item: Item, anchor_x: usize, anchor_y: usize) -> Option<ItemId> {
        if !self.can_place_item(&item, anchor_x, anchor_y) {
            return None;
        }

        let item_id = self.next_id;
        self.next_id += 1;

        let cells = item.current_occupied_cells();

        // Mark cells as occupied
        for (i, (dx, dy)) in cells.iter().enumerate() {
            let x = anchor_x + dx;
            let y = anchor_y + dy;

            self.grid[y][x] = InventoryCell {
                item_id: Some(item_id),
                is_anchor: i == 0, // First cell is anchor
            };
        }

        // Store the item
        if item_id >= self.items.len() {
            self.items.resize(item_id + 1, None);
        }
        self.items[item_id] = Some(item);

        Some(item_id)
    }

    /// Remove an item by its ID
    pub fn remove_item(&mut self, item_id: ItemId) -> Option<Item> {
        if item_id >= self.items.len() {
            return None;
        }

        let item = self.items[item_id].take()?;

        // Clear all cells occupied by this item
        for row in &mut self.grid {
            for cell in row {
                if cell.item_id == Some(item_id) {
                    *cell = InventoryCell::default();
                }
            }
        }

        Some(item)
    }

    /// Get item ID at a specific grid position
    pub fn get_item_id_at(&self, x: usize, y: usize) -> Option<ItemId> {
        if self.in_bounds(x, y) {
            self.grid[y][x].item_id
        } else {
            None
        }
    }

    /// Get item reference by ID
    pub fn get_item(&self, item_id: ItemId) -> Option<&Item> {
        self.items.get(item_id).and_then(|opt| opt.as_ref())
    }

    /// Get mutable item reference by ID
    pub fn get_item_mut(&mut self, item_id: ItemId) -> Option<&mut Item> {
        self.items.get_mut(item_id).and_then(|opt| opt.as_mut())
    }

    /// Get the anchor position of an item
    pub fn get_item_anchor(&self, item_id: ItemId) -> Option<(usize, usize)> {
        for (y, row) in self.grid.iter().enumerate() {
            for (x, cell) in row.iter().enumerate() {
                if cell.item_id == Some(item_id) && cell.is_anchor {
                    return Some((x, y));
                }
            }
        }
        None
    }

    /// Move an item to a new position
    pub fn move_item(&mut self, item_id: ItemId, new_anchor_x: usize, new_anchor_y: usize) -> bool {
        // Remove item temporarily
        let Some(item) = self.remove_item(item_id) else {
            return false;
        };

        // Try to place at new position
        if let Some(_new_id) = self.place_item(item.clone(), new_anchor_x, new_anchor_y) {
            // Success - update the stored item to use the new ID
            // (Note: this maintains the same item but with a new ID)
            true
        } else {
            // Failed - try to put it back at original position
            if let Some(orig_pos) = self.get_item_anchor(item_id) {
                self.place_item(item, orig_pos.0, orig_pos.1);
            }
            false
        }
    }

    /// Automatically add an item to the inventory, trying all rotations and positions
    /// Returns Some(ItemId) if successful, None if no space found
    pub fn add_item(&mut self, mut item: Item) -> Option<ItemId> {
        let original_rotation = item.rotation;

        // Try all 4 rotations (or just 1 if rotation is locked)
        let max_rotations = if item.rotation_locked { 1 } else { 4 };

        for rotation_attempt in 0..max_rotations {
            item.rotation = (original_rotation + rotation_attempt) % 4;

            // Scan through all grid positions from top-left to bottom-right
            for y in 0..self.height {
                for x in 0..self.width {
                    // Try to place item at this position
                    if let Some(item_id) = self.place_item(item.clone(), x, y) {
                        return Some(item_id);
                    }
                }
            }
        }

        // No space found in any rotation or position
        None
    }
}

impl Default for Inventory {
    fn default() -> Self {
        Self::new(10, 8) // 10 wide x 8 high for Tetris-style inventory
    }
}
