use super::OpIterator;
use crate::Managers;
use common::bytecode_expr::ByteCodeExpr;
use common::error::c_err;

use common::{CrustyError, Field, TableSchema, Tuple};

use std::cmp::{self, Ordering};

pub struct SortMergeJoin {
    // Static objects (No need to reset on close)
    managers: &'static Managers,

    // Parameters (No need to reset on close)
    schema: TableSchema,
    left_expr: Vec<(ByteCodeExpr, bool)>,
    right_expr: Vec<(ByteCodeExpr, bool)>,
    left_child: Box<dyn OpIterator>,
    right_child: Box<dyn OpIterator>,
    will_rewind: bool,
    // States (Reset on close)
    // todo!(Add the states you need to maintain here)
}

impl SortMergeJoin {
    pub fn new(
        managers: &'static Managers,
        schema: TableSchema,
        left_expr: Vec<(ByteCodeExpr, bool)>,
        right_expr: Vec<(ByteCodeExpr, bool)>,
        left_child: Box<dyn OpIterator>,
        right_child: Box<dyn OpIterator>,
    ) -> Result<Self, CrustyError> {
        if left_expr.len() != right_expr.len() {
            return Err(c_err(
                "SMJ: Left and right expressions must have the same length",
            ));
        }
        if left_expr.is_empty() {
            return Err(c_err("SMJ: Join predicate cannot be empty"));
        }
        todo!("Add your code");
    }
}

impl OpIterator for SortMergeJoin {
    fn configure(&mut self, will_rewind: bool) {
        self.will_rewind = will_rewind;
        // will_rewind is false for both children because the sort is stateful and rewinding sort operator does not rewind child
        self.left_child.configure(false);
        self.right_child.configure(false);
    }

    fn open(&mut self) -> Result<(), CrustyError> {
        todo!("Add your code");
    }

    fn next(&mut self) -> Result<Option<Tuple>, CrustyError> {
        todo!("Add your code");
    }

    fn close(&mut self) -> Result<(), CrustyError> {
        todo!("Add your code");
    }

    fn rewind(&mut self) -> Result<(), CrustyError> {
        todo!("Add your code");
    }

    fn get_schema(&self) -> &TableSchema {
        &self.schema
    }
}

#[cfg(test)]
mod test {
    use super::super::TupleIterator;
    use super::*;
    use crate::testutil::execute_iter;
    use crate::testutil::new_test_managers;
    use crate::testutil::TestTuples;
    use common::bytecode_expr::{ByteCodeExpr, ByteCodes};
    use common::Field;

    fn get_join_predicate() -> (Vec<(ByteCodeExpr, bool)>, Vec<(ByteCodeExpr, bool)>) {
        // Joining two tables each containing the following tuples:
        // 1 1 3 E
        // 2 1 3 G
        // 3 1 4 A
        // 4 2 4 G
        // 5 2 5 G
        // 6 2 5 G

        // left(col(0) + col(1)) OP right(col(2))
        let mut left = ByteCodeExpr::new();
        left.add_code(ByteCodes::PushField as usize);
        left.add_code(0);
        left.add_code(ByteCodes::PushField as usize);
        left.add_code(1);
        left.add_code(ByteCodes::Add as usize);

        let mut right = ByteCodeExpr::new();
        right.add_code(ByteCodes::PushField as usize);
        right.add_code(2);

        let left_expr = vec![(left, false)];
        let right_expr = vec![(right, false)];
        (left_expr, right_expr)
    }

    fn get_iter(
        left_expr: Vec<(ByteCodeExpr, bool)>,
        right_expr: Vec<(ByteCodeExpr, bool)>,
    ) -> Box<dyn OpIterator> {
        let setup = TestTuples::new("");
        let managers = new_test_managers();
        let mut iter = Box::new(
            SortMergeJoin::new(
                managers,
                setup.schema.clone(),
                left_expr,
                right_expr,
                Box::new(TupleIterator::new(
                    setup.tuples.clone(),
                    setup.schema.clone(),
                )),
                Box::new(TupleIterator::new(
                    setup.tuples.clone(),
                    setup.schema.clone(),
                )),
            )
            .unwrap(),
        );
        iter.configure(false);
        iter
    }

    fn run_sort_merge_join(
        left_expr: Vec<(ByteCodeExpr, bool)>,
        right_expr: Vec<(ByteCodeExpr, bool)>,
    ) -> Vec<Tuple> {
        let mut iter = get_iter(left_expr, right_expr);
        execute_iter(&mut *iter, true).unwrap()
    }

    mod sort_merge_join_test {
        use super::*;

        #[test]
        fn test_join() {
            // Joining two tables each containing the following tuples:
            // 1 1 3 E
            // 2 1 3 G
            // 3 1 4 A
            // 4 2 4 G
            // 5 2 5 G
            // 6 2 5 G

            // left(col(0) + col(1)) == right(col(2))

            // Output:
            // 2 1 3 G 1 1 3 E
            // 2 1 3 G 2 1 3 G
            // 3 1 4 A 3 1 4 A
            // 3 1 4 A 4 2 4 G
            let (left_expr, right_expr) = get_join_predicate();
            let t = run_sort_merge_join(left_expr, right_expr);
            assert_eq!(t.len(), 4);
            assert_eq!(
                t[0],
                Tuple::new(vec![
                    Field::Int(2),
                    Field::Int(1),
                    Field::Int(3),
                    Field::String("G".to_string()),
                    Field::Int(1),
                    Field::Int(1),
                    Field::Int(3),
                    Field::String("E".to_string()),
                ])
            );
            assert_eq!(
                t[1],
                Tuple::new(vec![
                    Field::Int(2),
                    Field::Int(1),
                    Field::Int(3),
                    Field::String("G".to_string()),
                    Field::Int(2),
                    Field::Int(1),
                    Field::Int(3),
                    Field::String("G".to_string()),
                ])
            );
            assert_eq!(
                t[2],
                Tuple::new(vec![
                    Field::Int(3),
                    Field::Int(1),
                    Field::Int(4),
                    Field::String("A".to_string()),
                    Field::Int(3),
                    Field::Int(1),
                    Field::Int(4),
                    Field::String("A".to_string()),
                ])
            );
            assert_eq!(
                t[3],
                Tuple::new(vec![
                    Field::Int(3),
                    Field::Int(1),
                    Field::Int(4),
                    Field::String("A".to_string()),
                    Field::Int(4),
                    Field::Int(2),
                    Field::Int(4),
                    Field::String("G".to_string()),
                ])
            );
        }
    }

    mod opiterator_test {
        use super::*;

        #[test]
        #[should_panic]
        fn test_next_not_open() {
            let (left_expr, right_expr) = get_join_predicate();
            let mut iter = get_iter(left_expr, right_expr);
            let _ = iter.next();
        }

        #[test]
        #[should_panic]
        fn test_rewind_not_open() {
            let (left_expr, right_expr) = get_join_predicate();
            let mut iter = get_iter(left_expr, right_expr);
            let _ = iter.rewind();
        }

        #[test]
        fn test_open() {
            let (left_expr, right_expr) = get_join_predicate();
            let mut iter = get_iter(left_expr, right_expr);
            iter.open().unwrap();
        }

        #[test]
        fn test_close() {
            let (left_expr, right_expr) = get_join_predicate();
            let mut iter = get_iter(left_expr, right_expr);
            iter.open().unwrap();
            iter.close().unwrap();
        }

        #[test]
        fn test_rewind() {
            let (left_expr, right_expr) = get_join_predicate();
            let mut iter = get_iter(left_expr, right_expr);
            iter.configure(true);
            let t_before = execute_iter(&mut *iter, false).unwrap();
            iter.rewind().unwrap();
            let t_after = execute_iter(&mut *iter, false).unwrap();
            assert_eq!(t_before, t_after);
        }
    }
}
