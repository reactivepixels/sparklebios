//! The cluster map's own state machine. Pure: no filesystem, no terminal, no clock beyond the
//! seed it is handed once, at construction. Everything about how the map fills in is decided
//! here, so it can all be tested without a real directory or a real tty.

/// One top level entry the map colours in: its display name and its real disk usage, in bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The entry's own file or directory name, not a full path.
    pub name: String,
    /// Real disk usage: the sum of every block the entry (and, for a directory, everything
    /// under it) actually occupies, not merely the apparent byte length of its content.
    pub bytes: u64,
}

/// The cluster map itself: the entries it is colouring in, which grid cell each belongs to, the
/// order cells reveal in, and how far that reveal has gotten.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Map {
    /// The entries this map colours in, largest first, at most six of them.
    pub entries: Vec<Entry>,
    /// Which entry (an index into `entries`) owns each grid cell, flattened row major.
    cell_owner: Vec<usize>,
    /// A permutation of every grid cell, the order they reveal in as the map fills.
    reveal_order: Vec<usize>,
    /// Whether each grid cell (by its own flat index, not its position in `reveal_order`) has
    /// been revealed yet.
    revealed_mask: Vec<bool>,
    /// How many of `reveal_order`'s cells have been revealed so far.
    revealed: usize,
}

impl Map {
    /// Builds a map over `grid` cells for `entries` (already sorted largest first and capped to
    /// six): cells are allocated to entries in proportion to their size (see `allocate`), and the
    /// order they reveal in is shuffled from `seed` (see `shuffled`) so the fill looks like it is
    /// happening rather than simply sweeping across the screen.
    pub fn new(entries: Vec<Entry>, grid: usize, seed: u64) -> Map {
        let sizes: Vec<u64> = entries.iter().map(|e| e.bytes).collect();
        let counts = allocate(&sizes, grid);
        let mut cell_owner = Vec::with_capacity(grid);
        for (i, &count) in counts.iter().enumerate() {
            cell_owner.extend(std::iter::repeat(i).take(count));
        }
        Map {
            entries,
            cell_owner,
            reveal_order: shuffled(grid, seed),
            revealed_mask: vec![false; grid],
            revealed: 0,
        }
    }

    /// The grid's own size: the number of cells this map was built over.
    pub fn grid(&self) -> usize {
        self.cell_owner.len()
    }

    /// Whether every cell has been revealed.
    pub fn done(&self) -> bool {
        self.revealed >= self.grid()
    }

    /// How far the fill has gotten, 0 to 100. A grid of zero cells (never built from a real
    /// directory, only possible in a test) reads as done rather than dividing by zero.
    pub fn percent(&self) -> u8 {
        let grid = self.grid();
        if grid == 0 {
            return 100;
        }
        ((self.revealed * 100) / grid) as u8
    }

    /// Reveals up to `amount` more cells, in `reveal_order`. Never overshoots the grid, so this
    /// can be called with a fixed step every frame without the caller having to also check
    /// `done`.
    pub fn step(&mut self, amount: usize) {
        let grid = self.grid();
        let new_revealed = (self.revealed + amount).min(grid);
        for &cell in &self.reveal_order[self.revealed..new_revealed] {
            self.revealed_mask[cell] = true;
        }
        self.revealed = new_revealed;
    }

    /// Which entry owns grid cell `flat_index`, or `None` if that cell has not been revealed yet.
    /// `flat_index` is the cell's own row major position, not its position in the reveal order.
    pub fn cell(&self, flat_index: usize) -> Option<usize> {
        if *self.revealed_mask.get(flat_index)? {
            self.cell_owner.get(flat_index).copied()
        } else {
            None
        }
    }
}

/// Splits `grid` cells across `sizes` in proportion to each one, by the largest remainder method:
/// every entry first gets the whole number of cells its exact share floors to, then whatever is
/// left over (there is always something left over unless every share divided evenly) goes one at
/// a time to the entries whose share was cut off closest to its next whole cell, largest first.
/// The result always sums to exactly `grid`, however awkwardly `sizes` divides into it: nothing
/// is left over, and nothing is counted twice.
///
/// `sizes.len() == 0` or every size zero returns a same length vector of zeroes rather than
/// dividing by zero; `Map::new` never actually calls this with either, since an all zero or empty
/// directory is turned away before a map is ever built (see `defrag::run`).
pub fn allocate(sizes: &[u64], grid: usize) -> Vec<usize> {
    let total: u128 = sizes.iter().map(|&s| s as u128).sum();
    if total == 0 {
        return vec![0; sizes.len()];
    }
    let grid = grid as u128;
    let shares: Vec<(u128, u128)> = sizes
        .iter()
        .map(|&s| {
            let s = s as u128;
            (grid * s / total, grid * s % total)
        })
        .collect();
    let mut cells: Vec<u128> = shares.iter().map(|&(whole, _)| whole).collect();
    let allocated: u128 = cells.iter().sum();
    let mut remaining = grid - allocated;

    let mut order: Vec<usize> = (0..sizes.len()).collect();
    order.sort_by(|&a, &b| {
        // Largest remainder first; ties broken by the larger size, then by the earlier entry, so
        // the result is fully deterministic rather than depending on an unstable sort's mood.
        shares[b]
            .1
            .cmp(&shares[a].1)
            .then_with(|| sizes[b].cmp(&sizes[a]))
            .then_with(|| a.cmp(&b))
    });
    for &i in &order {
        if remaining == 0 {
            break;
        }
        cells[i] += 1;
        remaining -= 1;
    }
    cells.into_iter().map(|c| c as usize).collect()
}

/// A splitmix64 generator, used only to shuffle the fill order deterministically from a seed.
/// Nothing here needs to be cryptographic, only repeatable.
struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

/// A Fisher-Yates shuffle of `0..n`, deterministic from `seed`: the same seed always produces the
/// same order, so a test can pin one exactly.
pub fn shuffled(n: usize, seed: u64) -> Vec<usize> {
    let mut values: Vec<usize> = (0..n).collect();
    let mut rng = Rng(seed);
    for i in (1..n).rev() {
        let j = (rng.next_u64() % (i as u64 + 1)) as usize;
        values.swap(i, j);
    }
    values
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, bytes: u64) -> Entry {
        Entry {
            name: name.to_string(),
            bytes,
        }
    }

    #[test]
    fn allocate_sums_to_exactly_the_grid_even_when_nothing_divides_evenly() {
        // 37 does not divide evenly by any of these; the classic case largest remainder exists
        // to solve. Every case is checked, not just the total.
        for (sizes, grid) in [
            (vec![1u64, 1, 1], 10),
            (vec![7, 5, 3], 15),
            (vec![1, 2, 3, 4, 5, 6], 37),
            (vec![1, 1, 1, 1, 1, 1], 100),
            (vec![100, 1, 1, 1, 1, 1], 7),
            (vec![333, 333, 334], 1),
        ] {
            let counts = allocate(&sizes, grid);
            let sum: usize = counts.iter().sum();
            assert_eq!(sum, grid, "{sizes:?} over {grid} cells: got {counts:?}");
        }
    }

    #[test]
    fn allocate_never_double_counts_a_cell() {
        // A regression pin: an earlier draft distributed the remainder without tracking how many
        // cells had already been handed out, which could double allocate the last one or two.
        let counts = allocate(&[10, 10, 10, 10, 10, 10, 10], 100);
        assert_eq!(counts.iter().sum::<usize>(), 100);
        assert_eq!(counts.len(), 7);
    }

    #[test]
    fn allocate_is_proportional_not_just_correctly_summed() {
        let counts = allocate(&[600, 300, 100], 100);
        assert_eq!(counts, vec![60, 30, 10]);
    }

    #[test]
    fn allocate_gives_the_larger_share_to_the_larger_remainder() {
        // 10 cells over sizes 5 and 5 splits evenly; over 5 and 4 it cannot, and the plan's own
        // reference (largest remainder) hands the leftover cell to whichever share was cut off
        // closer to its next whole cell.
        let counts = allocate(&[5, 4], 10);
        assert_eq!(counts.iter().sum::<usize>(), 10);
    }

    #[test]
    fn allocate_of_nothing_is_every_entry_getting_nothing() {
        assert_eq!(allocate(&[0, 0, 0], 50), vec![0, 0, 0]);
        assert_eq!(allocate(&[], 50), Vec::<usize>::new());
    }

    #[test]
    fn shuffled_is_a_permutation_not_a_new_set_of_numbers() {
        let mut order = shuffled(50, 12345);
        order.sort_unstable();
        assert_eq!(order, (0..50).collect::<Vec<_>>());
    }

    #[test]
    fn shuffled_is_deterministic_from_its_seed() {
        assert_eq!(shuffled(50, 7), shuffled(50, 7));
    }

    #[test]
    fn shuffled_of_zero_is_empty() {
        assert_eq!(shuffled(0, 7), Vec::<usize>::new());
    }

    #[test]
    fn a_fresh_map_has_revealed_nothing_and_is_not_done() {
        let map = Map::new(vec![entry("src", 100), entry("docs", 50)], 20, 1);
        assert_eq!(map.percent(), 0);
        assert!(!map.done());
        assert!((0..map.grid()).all(|i| map.cell(i).is_none()));
    }

    #[test]
    fn stepping_reveals_cells_and_eventually_finishes() {
        let mut map = Map::new(vec![entry("src", 100), entry("docs", 50)], 20, 1);
        map.step(5);
        assert_eq!(map.percent(), 25);
        assert_eq!(
            (0..map.grid()).filter(|&i| map.cell(i).is_some()).count(),
            5
        );
        map.step(15);
        assert!(map.done());
        assert_eq!(map.percent(), 100);
        assert!((0..map.grid()).all(|i| map.cell(i).is_some()));
    }

    #[test]
    fn stepping_past_the_grid_does_not_overshoot() {
        let mut map = Map::new(vec![entry("src", 100)], 10, 1);
        map.step(1000);
        assert!(map.done());
        assert_eq!(map.grid(), 10);
    }

    #[test]
    fn every_revealed_cell_belongs_to_exactly_one_entry_in_proportion_to_its_size() {
        let map = Map::new(vec![entry("big", 900), entry("small", 100)], 100, 42);
        let mut map = map;
        map.step(100);
        let big_cells = (0..map.grid()).filter(|&i| map.cell(i) == Some(0)).count();
        let small_cells = (0..map.grid()).filter(|&i| map.cell(i) == Some(1)).count();
        assert_eq!(big_cells + small_cells, 100);
        assert_eq!(big_cells, 90);
        assert_eq!(small_cells, 10);
    }
}
