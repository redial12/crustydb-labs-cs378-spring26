use super::OpIterator;
use crate::Managers;
use common::bytecode_expr::ByteCodeExpr;
use common::datatypes::f_decimal;
use common::{AggOp, CrustyError, Field, TableSchema, Tuple};
use std::cmp::{max, min};
use std::collections::HashMap;

/// Aggregate operator. (You can add any other fields that you think are neccessary)
pub struct Aggregate {
    // Static objects (No need to reset on close)
    managers: &'static Managers,

    // Parameters (No need to reset on close)
    /// Output schema of the form [groupby_field attributes ..., agg_field attributes ...]).
    schema: TableSchema,
    /// Group by fields
    groupby_expr: Vec<ByteCodeExpr>,
    /// Aggregated fields.
    agg_expr: Vec<ByteCodeExpr>,
    /// Aggregation operations.
    ops: Vec<AggOp>,
    /// Child operator to get the data from.
    child: Box<dyn OpIterator>,
    /// If true, then the operator will be rewinded in the future.
    will_rewind: bool,

    // States (Need to reset on close)
    open: bool,
    /// map used to aggregate
    map: HashMap<Vec<Field>, Vec<Field>>,
    /// ops expanded so every Avg is followed by a Count slot
    ops_other: Vec<AggOp>,
    /// results collected into here from map
    results: Vec<Tuple>,
    /// manages index for next()
    index: usize,
}

// thoughts: use a hash map to store mapping from groups to values
// map a vec of fields to a vec of fields
// the map needs to be built in open()
// check if a group exists, if so then merge fields.
// merge fields by iterating over the values(vec of Fields) from key and calling merge on each one
// if group doesnt exist already, call place_fields method (similar to merge)
// place_fields will handle the first insertion correctly for each Op, for Avg create an extra field for count

//for next() I need to turn the hashmap into tuples

impl Aggregate {
    pub fn new(
        managers: &'static Managers,
        groupby_expr: Vec<ByteCodeExpr>,
        agg_expr: Vec<ByteCodeExpr>,
        ops: Vec<AggOp>,
        schema: TableSchema,
        child: Box<dyn OpIterator>,
    ) -> Self {
        assert!(ops.len() == agg_expr.len());
        Self {
            managers,
            groupby_expr,
            agg_expr,
            ops,
            schema,
            child,
            open: false,
            map: HashMap::new(),
            ops_other: Vec::new(),
            results: Vec::new(),
            index: 0,
            will_rewind: false,
        }
    }

    fn merge_fields(op: AggOp, field_val: &Field, acc: &mut Field) -> Result<(), CrustyError> {
        match op {
            AggOp::Count => *acc = (acc.clone() + Field::Int(1))?,
            AggOp::Max => {
                let max = max(acc.clone(), field_val.clone());
                *acc = max;
            }
            AggOp::Min => {
                let min = min(acc.clone(), field_val.clone());
                *acc = min;
            }
            AggOp::Sum => {
                *acc = (acc.clone() + field_val.clone())?;
            }
            AggOp::Avg => {
                *acc = (acc.clone() + field_val.clone())?; // This will be divided by the count later
            }
        }
        Ok(())
    }

    fn place_fields(op: AggOp, field_val: &Field) -> Field {
        match op {
            AggOp::Count => return Field::Int(1),
            AggOp::Max => return field_val.clone(),
            AggOp::Min => return field_val.clone(),
            AggOp::Sum => return field_val.clone(),
            AggOp::Avg => match field_val {
                Field::Int(v) => return f_decimal(*v as f64),
                other => return other.clone(),
            },
        }
    }

    pub fn merge_tuple_into_group(&mut self, tuple: &Tuple) {
        // get group by fields of tuple in vec
        let mut groupby_fields: Vec<Field> = Vec::new();

        for expr in self.groupby_expr.iter() {
            let field = expr.eval(&tuple);
            groupby_fields.push(field.clone());
        }

        // get agg fields of tuple in vec
        let mut agg_fields: Vec<Field> = Vec::new();

        for i in 0..self.agg_expr.len() {
            let expr = &self.agg_expr[i];
            let field = expr.eval(&tuple);
            let op = self.ops[i];

            //push twice for avg
            if op == AggOp::Avg {
                agg_fields.push(field.clone());
            }

            agg_fields.push(field.clone());
        }

        // check if entry exists
        if let Some(fields) = self.map.get_mut(&groupby_fields) {
            // merge each agg_field
            for i in 0..fields.len() {
                let _ = Self::merge_fields(self.ops_other[i], &agg_fields[i], &mut fields[i]);
            }
        } else {
            // call place field
            let mut init_fields: Vec<Field> = Vec::new();

            for i in 0..agg_fields.len() {
                let field = agg_fields[i].clone();
                init_fields.push(Self::place_fields(self.ops_other[i], &field))
            }

            self.map.insert(groupby_fields, init_fields);
        }
    }
}

impl OpIterator for Aggregate {
    fn configure(&mut self, will_rewind: bool) {
        self.will_rewind = will_rewind;
        self.child.configure(false); // child of a aggregate will never be rewinded
                                     // because aggregate will buffer all the tuples from the child
    }

    fn open(&mut self) -> Result<(), CrustyError> {
        if !self.open {
            self.child.open()?;
            self.open = true;

            // build ops_other: expand each Avg into [Avg, Count]
            for op in self.ops.iter() {
                self.ops_other.push(*op);
                if *op == AggOp::Avg {
                    self.ops_other.push(AggOp::Count);
                }
            }

            // build hashmap
            while let Some(tuple) = self.child.next()? {
                self.merge_tuple_into_group(&tuple);
            }

            // collect, sort by group key, finalize AVG into results
            let mut entries: Vec<(Vec<Field>, Vec<Field>)> = self.map.drain().collect();
            entries.sort_by(|(a, _), (b, _)| a.cmp(b));

            for (group_key, agg_vals) in entries {
                let mut fields = group_key;
                let mut i = 0;
                while i < agg_vals.len() {
                    if self.ops_other[i] == AggOp::Avg {
                        // agg_vals[i] = sum (Decimal), agg_vals[i+1] = count (Int)
                        let avg = (agg_vals[i].clone() / agg_vals[i + 1].clone())?;
                        fields.push(avg);
                        i += 2;
                    } else {
                        fields.push(agg_vals[i].clone());
                        i += 1;
                    }
                }
                self.results.push(Tuple::new(fields));
            }
        }
        Ok(())
    }

    fn next(&mut self) -> Result<Option<Tuple>, CrustyError> {
        if !self.open {
            panic!("Iterator is not open");
        }
        if self.index < self.results.len() {
            let t = self.results[self.index].clone();
            self.index += 1;
            Ok(Some(t))
        } else {
            Ok(None)
        }
    }

    fn close(&mut self) -> Result<(), CrustyError> {
        self.child.close()?;
        self.open = false;
        self.index = 0;
        self.map.clear();
        self.ops_other.clear();
        self.results.clear();
        Ok(())
    }

    fn rewind(&mut self) -> Result<(), CrustyError> {
        if !self.open {
            panic!("Iterator is not open");
        }
        self.index = 0;
        Ok(())
    }

    fn get_schema(&self) -> &TableSchema {
        &self.schema
    }
}

#[cfg(test)]
mod test {
    use super::super::TupleIterator;
    use super::*;
    use crate::testutil::{execute_iter, new_test_managers, TestTuples};
    use common::{
        bytecode_expr::colidx_expr,
        datatypes::{f_int, f_str},
    };

    fn get_iter(
        groupby_expr: Vec<ByteCodeExpr>,
        agg_expr: Vec<ByteCodeExpr>,
        ops: Vec<AggOp>,
    ) -> Box<dyn OpIterator> {
        let setup = TestTuples::new("");
        let managers = new_test_managers();
        let dummy_schema = TableSchema::new(vec![]);
        let mut iter = Box::new(Aggregate::new(
            managers,
            groupby_expr,
            agg_expr,
            ops,
            dummy_schema,
            Box::new(TupleIterator::new(
                setup.tuples.clone(),
                setup.schema.clone(),
            )),
        ));
        iter.configure(false);
        iter
    }

    fn run_aggregate(
        groupby_expr: Vec<ByteCodeExpr>,
        agg_expr: Vec<ByteCodeExpr>,
        ops: Vec<AggOp>,
    ) -> Vec<Tuple> {
        let mut iter = get_iter(groupby_expr, agg_expr, ops);
        execute_iter(&mut *iter, true).unwrap()
    }

    mod aggregation_test {
        use super::*;

        #[test]
        fn test_empty_group() {
            let group_by = vec![];
            let agg = vec![colidx_expr(0), colidx_expr(1), colidx_expr(2)];
            let ops = vec![AggOp::Count, AggOp::Max, AggOp::Avg];
            let t = run_aggregate(group_by, agg, ops);
            assert_eq!(t.len(), 1);
            assert_eq!(t[0], Tuple::new(vec![f_int(6), f_int(2), f_decimal(4.0)]));
        }

        #[test]
        fn test_empty_aggregation() {
            let group_by = vec![colidx_expr(2)];
            let agg = vec![];
            let ops = vec![];
            let t = run_aggregate(group_by, agg, ops);
            assert_eq!(t.len(), 3);
            assert_eq!(t[0], Tuple::new(vec![f_int(3)]));
            assert_eq!(t[1], Tuple::new(vec![f_int(4)]));
            assert_eq!(t[2], Tuple::new(vec![f_int(5)]));
        }

        #[test]
        fn test_count() {
            // Input:
            // 1 1 3 E
            // 2 1 3 G
            // 3 1 4 A
            // 4 2 4 G
            // 5 2 5 G
            // 6 2 5 G
            let group_by = vec![colidx_expr(1), colidx_expr(2)];
            let agg = vec![colidx_expr(0)];
            let ops = vec![AggOp::Count];
            let t = run_aggregate(group_by, agg, ops);
            // Output:
            // 1 3 2
            // 1 4 1
            // 2 4 1
            // 2 5 2
            assert_eq!(t.len(), 4);
            assert_eq!(t[0], Tuple::new(vec![f_int(1), f_int(3), f_int(2)]));
            assert_eq!(t[1], Tuple::new(vec![f_int(1), f_int(4), f_int(1)]));
            assert_eq!(t[2], Tuple::new(vec![f_int(2), f_int(4), f_int(1)]));
            assert_eq!(t[3], Tuple::new(vec![f_int(2), f_int(5), f_int(2)]));
        }

        #[test]
        fn test_sum() {
            // Input:
            // 1 1 3 E
            // 2 1 3 G
            // 3 1 4 A
            // 4 2 4 G
            // 5 2 5 G
            // 6 2 5 G

            let group_by = vec![colidx_expr(1), colidx_expr(2)];
            let agg = vec![colidx_expr(0)];
            let ops = vec![AggOp::Sum];
            let tuples = run_aggregate(group_by, agg, ops);
            // Output:
            // 1 3 3
            // 1 4 3
            // 2 4 4
            // 2 5 11
            assert_eq!(tuples.len(), 4);
            assert_eq!(tuples[0], Tuple::new(vec![f_int(1), f_int(3), f_int(3)]));
            assert_eq!(tuples[1], Tuple::new(vec![f_int(1), f_int(4), f_int(3)]));
            assert_eq!(tuples[2], Tuple::new(vec![f_int(2), f_int(4), f_int(4)]));
            assert_eq!(tuples[3], Tuple::new(vec![f_int(2), f_int(5), f_int(11)]));
        }

        #[test]
        fn test_max() {
            // Input:
            // 1 1 3 E
            // 2 1 3 G
            // 3 1 4 A
            // 4 2 4 G
            // 5 2 5 G
            // 6 2 5 G

            let group_by = vec![colidx_expr(1), colidx_expr(2)];
            let agg = vec![colidx_expr(3)];
            let ops = vec![AggOp::Max];
            let t = run_aggregate(group_by, agg, ops);
            // Output:
            // 1 3 G
            // 1 4 A
            // 2 4 G
            // 2 5 G
            assert_eq!(t.len(), 4);
            assert_eq!(t[0], Tuple::new(vec![f_int(1), f_int(3), f_str("G")]));
            assert_eq!(t[1], Tuple::new(vec![f_int(1), f_int(4), f_str("A")]));
            assert_eq!(t[2], Tuple::new(vec![f_int(2), f_int(4), f_str("G")]));
            assert_eq!(t[3], Tuple::new(vec![f_int(2), f_int(5), f_str("G")]));
        }

        #[test]
        fn test_min() {
            // Input:
            // 1 1 3 E
            // 2 1 3 G
            // 3 1 4 A
            // 4 2 4 G
            // 5 2 5 G
            // 6 2 5 G

            let group_by = vec![colidx_expr(1), colidx_expr(2)];
            let agg = vec![colidx_expr(3)];
            let ops = vec![AggOp::Min];
            let t = run_aggregate(group_by, agg, ops);
            // Output:
            // 1 3 E
            // 1 4 A
            // 2 4 G
            // 2 5 G
            assert!(t.len() == 4);
            assert_eq!(t[0], Tuple::new(vec![f_int(1), f_int(3), f_str("E")]));
            assert_eq!(t[1], Tuple::new(vec![f_int(1), f_int(4), f_str("A")]));
            assert_eq!(t[2], Tuple::new(vec![f_int(2), f_int(4), f_str("G")]));
            assert_eq!(t[3], Tuple::new(vec![f_int(2), f_int(5), f_str("G")]));
        }

        #[test]
        fn test_avg() {
            // Input:
            // 1 1 3 E
            // 2 1 3 G
            // 3 1 4 A
            // 4 2 4 G
            // 5 2 5 G
            // 6 2 5 G
            let group_by = vec![colidx_expr(1), colidx_expr(2)];
            let agg = vec![colidx_expr(0)];
            let ops = vec![AggOp::Avg];
            let t = run_aggregate(group_by, agg, ops);
            // Output:
            // 1 3 1.5
            // 1 4 3.0
            // 2 4 4.0
            // 2 5 5.5
            assert_eq!(t.len(), 4);
            assert_eq!(t[0], Tuple::new(vec![f_int(1), f_int(3), f_decimal(1.5)]));
            assert_eq!(t[1], Tuple::new(vec![f_int(1), f_int(4), f_decimal(3.0)]));
            assert_eq!(t[2], Tuple::new(vec![f_int(2), f_int(4), f_decimal(4.0)]));
            assert_eq!(t[3], Tuple::new(vec![f_int(2), f_int(5), f_decimal(5.5)]));
        }

        #[test]
        fn test_multi_column_aggregation() {
            // Input:
            // 1 1 3 E
            // 2 1 3 G
            // 3 1 4 A
            // 4 2 4 G
            // 5 2 5 G
            // 6 2 5 G
            let group_by = vec![colidx_expr(3)];
            let agg = vec![colidx_expr(0), colidx_expr(1), colidx_expr(2)];
            let ops = vec![AggOp::Count, AggOp::Max, AggOp::Avg];
            let t = run_aggregate(group_by, agg, ops);
            // Output:
            // A 1 1 4.0
            // E 1 1 3.0
            // G 4 2 4.25
            assert_eq!(t.len(), 3);
            assert_eq!(
                t[0],
                Tuple::new(vec![f_str("A"), f_int(1), f_int(1), f_decimal(4.0)])
            );
            assert_eq!(
                t[1],
                Tuple::new(vec![f_str("E"), f_int(1), f_int(1), f_decimal(3.0)])
            );
            assert_eq!(
                t[2],
                Tuple::new(vec![f_str("G"), f_int(4), f_int(2), f_decimal(4.25)])
            );
        }

        #[test]
        #[should_panic]
        fn test_merge_tuples_not_int() {
            let group_by = vec![];
            let agg = vec![colidx_expr(3)];
            let ops = vec![AggOp::Avg];
            let _ = run_aggregate(group_by, agg, ops);
        }
    }

    mod opiterator_test {
        use super::*;

        #[test]
        #[should_panic]
        fn test_next_not_open() {
            let mut iter = get_iter(vec![], vec![], vec![]);
            let _ = iter.next();
        }

        #[test]
        #[should_panic]
        fn test_rewind_not_open() {
            let mut iter = get_iter(vec![], vec![], vec![]);
            let _ = iter.rewind();
        }

        #[test]
        fn test_open() {
            let mut iter = get_iter(vec![], vec![], vec![]);
            iter.open().unwrap();
        }

        #[test]
        fn test_close() {
            let mut iter = get_iter(vec![], vec![], vec![]);
            iter.open().unwrap();
            iter.close().unwrap();
        }

        #[test]
        fn test_rewind() {
            let mut iter = get_iter(vec![colidx_expr(2)], vec![colidx_expr(0)], vec![AggOp::Max]);
            iter.configure(true); // if we will rewind in the future, then we set will_rewind to true
            let t_before = execute_iter(&mut *iter, true).unwrap();
            iter.rewind().unwrap();
            let t_after = execute_iter(&mut *iter, true).unwrap();
            assert_eq!(t_before, t_after);
        }
    }
}
