//! # Sudoku Solver Module
//!
//! This module defines the Sudoku structure and contains the logic
//! for solving Sudoku puzzles using backtracking and constraint propagation.

//use embassy_rp::peripherals::PIO1;
//use embassy_rp::pio::StateMachine;

/// Possible errors during Sudoku parsing or solving.
pub enum SudokuError {
    InvalidFormat,
    InvalidNumber,
    NotEnoughArguments,
    NoSolution,
}

impl core::fmt::Debug for SudokuError {
    /// Formats the error for display.
    ///
    /// # Arguments
    ///
    /// * `f` - Formatter instance
    ///
    /// # Returns
    ///
    /// `core::fmt::Result`
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            SudokuError::InvalidFormat => write!(f, "Invalid format"),
            SudokuError::InvalidNumber => write!(f, "Invalid number"),
            SudokuError::NotEnoughArguments => write!(f, "Not enough arguments"),
            SudokuError::NoSolution => write!(f, "No solution found"),
        }
    }
}

/// Sudoku puzzle representation.
#[derive(Clone, Default)]
pub struct Sudoku {
    /// 9x9 grid containing the puzzle values (0 represents empty cells)
    pub grid: [[u8; 9]; 9],
}

impl Sudoku {
    /// Parses a Sudoku schema from a string.
    ///
    /// The schema should contain 9 rows separated by spaces, with each row
    /// containing 9 comma-separated values. Use `_` for empty cells.
    ///
    /// # Arguments
    ///
    /// * `schema` - String containing the puzzle schema
    ///
    /// # Returns
    ///
    /// * `Ok(())` - If parsing succeeds
    /// * `Err(SudokuError)` - If parsing fails
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let mut sudoku = Sudoku::default();
    /// sudoku.parse("5,3,_,_,7,_,_,_,_ 6,_,_,1,9,5,_,_,_ ...")?;
    /// ```
    pub fn parse(&mut self, schema: &str) -> Result<(), SudokuError> {
        let lines = schema.split(" ").collect::<heapless::Vec<&str, 9>>();

        if lines.len() != 9 {
            return Err(SudokuError::NotEnoughArguments);
        }

        for (i, line) in lines.iter().enumerate() {
            let numbers: heapless::Vec<u8, 18> = line
                .split(',')
                .map(|s| {
                    if s.trim() == "_" {
                        Ok(0)
                    } else {
                        s.trim()
                            .parse::<u8>()
                            .map_err(|_e| SudokuError::InvalidNumber)
                    }
                })
                .collect::<Result<_, _>>()?;

            if numbers.len() != 9 {
                return Err(SudokuError::InvalidNumber);
            }

            self.grid[i] = numbers
                .as_slice()
                .try_into()
                .map_err(|_| SudokuError::InvalidFormat)?;
        }

        Ok(())
    }

    /// Solves the Sudoku puzzle using optimized backtracking with constraint tracking.
    ///
    /// This implementation uses three constraint arrays (rows, columns, boxes) to
    /// track which numbers are already used, making the solution significantly faster
    /// than naive backtracking.
    ///
    /// # Returns
    ///
    /// * `Ok(())` - If a solution is found
    /// * `Err(SudokuError::NoSolution)` - If no solution exists
    ///
    /// # Algorithm
    ///
    /// Uses recursive backtracking with constraint propagation:
    /// - Maintains boolean arrays for used numbers in each row, column, and 3x3 box
    /// - Only tries valid numbers at each position
    /// - Backtracks when no valid number can be placed
    pub fn solve_fast(&mut self) -> Result<(), SudokuError> {
        let mut rows = [[false; 10]; 9];
        let mut cols = [[false; 10]; 9];
        let mut boxes = [[false; 10]; 9];

        // Initialize constraints based on existing numbers
        for (row, grid_row) in self.grid.iter().enumerate() {
            for (col, _) in grid_row.iter().enumerate() {
                let num = self.grid[row][col];
                if num != 0 {
                    let b = (row / 3) * 3 + (col / 3);
                    rows[row][num as usize] = true;
                    cols[col][num as usize] = true;
                    boxes[b][num as usize] = true;
                }
            }
        }

        fn solve_rec(
            grid: &mut [[u8; 9]; 9],
            rows: &mut [[bool; 10]; 9],
            cols: &mut [[bool; 10]; 9],
            boxes: &mut [[bool; 10]; 9],
        ) -> bool {
            for row in 0..9 {
                for col in 0..9 {
                    if grid[row][col] == 0 {
                        let b = (row / 3) * 3 + (col / 3);
                        for num in 1..=9 {
                            if !rows[row][num] && !cols[col][num] && !boxes[b][num] {
                                grid[row][col] = num as u8;
                                rows[row][num] = true;
                                cols[col][num] = true;
                                boxes[b][num] = true;
                                if solve_rec(grid, rows, cols, boxes) {
                                    return true;
                                }
                                grid[row][col] = 0;
                                rows[row][num] = false;
                                cols[col][num] = false;
                                boxes[b][num] = false;
                            }
                        }
                        return false;
                    }
                }
            }
            true
        }

        if solve_rec(&mut self.grid, &mut rows, &mut cols, &mut boxes) {
            Ok(())
        } else {
            Err(SudokuError::NoSolution)
        }
    }
}
