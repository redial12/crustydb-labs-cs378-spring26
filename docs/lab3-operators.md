_Due Date: Thursday, April 16th, 2026 at 11:59 pm_

**Important: make sure you use the up-to-date files in the `src/queryexe/src/opiterator` folder 
before starting this and the bonus lab!**

In this lab, you will implement the *groupby-aggregate* (`src/queryexe/src/opiterator/aggregate.rs`), and *join* operators
(`src/queryexe/src/opiterator/hash_join.rs` and `src/queryexe/src/opiterator/nested_loop_join.rs`), 
so you can execute more complex queries. 
As in the previous labs, we provide a suite of tests that your implementation must pass.

## Query Operators' Logic

The very first thing that you need to have clear is how the join, aggregate, and
groupby operators work. This is independent of CrustyDB. You should think hard
what each of these operators needs to do with the data that is fed to them for
them to produce the correct output. It probably helps writing some pseudocode on
paper, sketching a quick implementation in your favorite scripting language, or
just having a clear idea of the algorithm behind each of these operators. In
general, conceptually, each logical operator is simple. In practice,
implementations can get arbitrarily complicated, depending on how much you care
about optimizing for performance, for example. 

In this lab, we are not measuring performance, just correctness. 
Also, we assume all data can fit in memory. So you do not need to 
implement disk-based algorithms.

## Execution Engine

CrustyDB's execution engine adopts the iterator model (also called Volcano model) 
we have learned in the class and 
provides the following interfaces *open*, *next*, *close*, *rewind*, etc. 
This means that your implementation of *aggregate*, *groupby*, and *join* will need to implement this
interface (we have a Trait, OpIterator, in the Rust-based CrustyDB
implementation) as well so it can be integrated within CrustyDB's execution
engine. 

Hint. Take a look at how SQL queries get parsed and translated into logical
query plans (`queryexe/query/translate_and_validate.rs`). Then, take a look at how
these plans are executed by studying `queryexe/query/executor.rs`.

## OpIterator Trait

We use Rust's Trait system to represent the Volcano-style operator interfaces.
You can find the definition in the OpIterator Trait
(`queryexe/opiterator/mod.rs`), which every operator in CrustyDB implements.
You should read the comments on each interface to understand 
what they do.

For the *rewind* interface, it resets the operator such 
that the next *next()* call will return the first tuple of this operator. 
However, it does not mean you need to compute the state from scratch. 
For example, for the hash join, you do not need to clean the hash table 
for *rewind()*. You only need to probe the hash table using the first tuple 
from the other subtree such that it generates the first tuple for this hash join.

Furthermore, you should take a look at the operators we have implemented for you
to understand how this interface is used in practice.

If you have set up a debugger (e.g., for setting up a breakpoint), 
this is a great time to put it to use: it'll help
you trace what happens during query execution 
(debuggers are not only useful to find bugs).

After you have understood the lifecycle of query execution, and once you have a
clear idea of what the *aggregate*, *groupby*, and *join* operators must do,
then it is time to implement them!

## Implementing Aggregate and Joins

Unlike a sequential scan, aggregate and join operators are *stateful* and
*blocking*. They are stateful because their output depends on their input and on
some *state* that the operator must manage. They are blocking because they
cannot produce output until they have seen the entire input data. If these two
concepts seem difficult, then I encourage you to write in pseudocode the
aggregate and join operators before jumping into the real implementation. These
two ideas should be very clear in your mind!

With that established, here's a basic set of directions to help you get started:

### Step 1: Read filter.rs

Filter.rs is a file that successfully implements OpIterator.rs. It's located in
`src/opiterator`, next to the other Rust files for SQL commands. You’ll notice the
following components:

- FilterPredicate
- Filter
- impl OpIterator for Filter

FilterPredicate is implemented as a bytecode that can be evaluated against a tuple.
The result of a bytecode is a Field, which can be any datatype that is supported by
the database. For example, if you have a record with 4 columns a, b, c, and d, and 
you want to evaluate the following expression: (a+b)-(c+d), you can represent it as
a [a][b][+][c][d][+][-] in bytecode. The bytecode is evaluated from left to right in 
a stack-based manner.

(a+b)-(c+d) Bytecode will be [a][b][+][c][d][+][-]
i, Stack
0, [a]
1, [a][b]
2, [a+b]
3, [a+b][c]
4, [a+b][c][d]
5, [a+b][c+d]
6, [a+b-c-d]

To finish the project, you don't have to understand how the bytecode works internally.
However, you should understand how to evaluate the bytecode against a tuple and what
the result of the evaluation is. 

In general, an expression in CrustyDB is represented as a `ByteCodeExpr` type. 
This expression is evaluated using its `eval()` interface, which takes a tuple 
as input and returns a field. See `src/common/src/bytecode_expr.rs` for the definition 
of `ByteCodeExpr`.

### Step 2: Implement NestedLoopJoin

Once you have the same logic as filter.rs, you'll probably notice that Filter is
designed for dealing with one child/entry at a time... but for obvious reasons,
Join needs to work on two at a time. This part is where you'll have to innovate!

You only need to implement the naive nested loop join as we assume we can hold all data in memory. 
NestedLoopJoin has `left_expr` and `right_expr` that must be evaluated against the 
left and right tuples before they are joined. `op` describes how to join the evaluated results. 
You can use the `ByteCodeExpr.eval` interface to extract the left and right join fields 
from left and right input tuples using `left_expr` and `right_expr`, respectively. 
You can use `src/common/src/datatypes.rs:compare_fields` to compare left and right join fields. 

You may also refer to the `src/queryexe/src/opiterator/cross_join.rs` for a simple
example of a join operator. You should pay attend to how they use *rewind* to support nested loops.

### Step 3: Implement HashEqJoin

The good news is that HashEqJoin isn't all that different from NestedLoopJoin--in fact,
now that you have the underlying logic for a NestedLoopJoin, HashEqJoin should be a lot
simpler! 
The class lecture slides should detail the basic concept of a hash equi-join: 
instead of using a highly inefficient nested loop to compare every
possible set of tuples, a hash equi-join does the following:

- Create a *hash table* out of one of the tables (Rust's HashMap data structure
  may be useful for this) based on each tuple's join key. 
  The value in this table should be a list of tuples associated with a join key (e.g., `vec<Tuple>`).
  This is because you need to design this *hash table* to store ALL tuples for a join key. 
  So insert a tuple into the hash table does not overwrite the previous tuple 
  that was in the hash table and has the same join key.

- Iterate over the other table, hash each tuple's join key, and compare 
its join key with the tuple's join key in this hash table. 
If the tuple's key is equal to a tuple's join key in the hash table, 
then it is one or multiple matches! 
So you will generate the joined tuple from the scanned tuple 
and the list of tuples associated with the join key. 

With the two joins done, you're ready to move on to aggregate.rs.

### Step 4: Implement Aggregate

Don't just rush into aggregate.rs--**make a plan first!**. Think about what
states need to be maintained, and what needs to be done with each tuple that
comes in.

The `groupby_expr` contains the bytecode that must be evaluated against the tuple 
to determine the group. A group may be represented by multiple fields. 
The `agg_expr` contains the bytecode that must be evaluated
against the tuple to determine the aggregation values. 
`ops` contains the aggregation operators that must be applied to the aggregation values.

Let's say you have a SQL query with the following structure:
```
SELECT SUM(a+b), COUNT(c-d) FROM table GROUP BY a, b+c
```
Then, conceptually, the `groupby_expr` has `[a, b+c]` and the `agg_expr` has `[a+b, c-d]`.
The `ops` will have `SUM` and `COUNT`. In this task, you will need to figure out
when to evaluate these expressions and how to store the results. Notice that the index of
the `agg_expr` corresponds to the index of the `ops`.

To make it easier, we have provided a skeleton for you to fill in. You will need to
implement `merge_tuple_into_group`: Given a tuple, this function must determine:
- Whether to add it to an existing group or create a new group, based on the
  specified groupby_fields in Aggregator
- How the value(s) stored within the aggregation fields should be aggregated,
  based on the specified agg_fields in Aggregator. 

There are five aggregation operators: SUM, COUNT, MIN, MAX, and AVG. AVG might be
tricky because it requires keeping track of both the sum and the count of the 
values in a group. 
We have provided the `merge_fields` function, which 
aggregates an input field into an accumulating field. 

I suggest you using Hashing-based Aggregation instead of Sorting-based. 
You could carefully design a single hash table to support all 5 aggregation operators. 

To help with your planning, here's a sample case to consider:

    You are running an online flower store, and have collected a database of the following customer information:

    Name    State   Variety     Price
    Alice   Alaska  Tulips      $10
    Alice   Nevada  Roses       $5
    Bob     Alaska  Tulips      $9
    Bob     Nevada  Daffodils   $7
    Bob     Nevada  Roses       $6

    1. You want to know the average order price in each state. Which groupby/aggregate statement(s) would you use?
    2. You want to know the average price of each variety of flowers. Which groupby/aggregate statement(s) would you use?
    3. You want to know the name that comes first in the alphabet for each state. Which groupby/aggregate statement(s) would you use?
    4. You want to know the total sum of how much each person has spent on flowers, and how many orders they've placed. (Assume each name/state combo is a unique person.) Which groupby/aggregate statement(s) would you use?
    
I would suggest thinking through these cases, or writing them down. What fields
will you need in your struct to handle all the cases? What assumptions are you
making? Remember: you can have multiple groupby fields *and* multiple aggregate
fields in the same call!

Once your `merge_tuple_into_group` is finished, you need to implement OpIterator, 
which shouldn't be hard.  

## Scoring and Requirements

### Testing

**Correctness**:
80% of your score on this lab is based on correctness. 
You can use the following commands to test your code:
`cargo test -p queryexe nested_loop_join` (7 tests), 
`cargo test -p queryexe hash_join` (7 tests), 
and `cargo test -p queryexe aggregate` (14 tests).

### Quality
10% of your score is based on code quality (following good coding conventions, comments, well-organized functions, etc). We will be looking for the following:

1. **Comments**: You should have comments for all new helper functions, constants and other identifiers that you add.
2. **Proper Types**: You should use suitable custom types. For example, you should use `SlotId` instead of `u16` when referring to a slot number. 
3. **Magic Numbers**: You should avoid magic numbers in your code. If you have a constant that is used in multiple places, you should define it as a constant at the top of the file.
4. **Descriptive Names**: Ensure that variables, functions, and constants have descriptive names that convey their purpose. Please don't use single-letter names or abbreviations unless they are widely recognized and contextually appropriate.

You could use `cargo fmt` to format your code in the right "style" and use 
`cargo clippy` to identify issues about your code, for either performance reasons or code quality. 
 
### Write Up
10% is based on your write up (`docs/lab3-writeup.txt`). The write up should contain:
 -  A brief description of your solution, in particular what design decisions you took and why. This is only needed for part of your solutions that had some significant work (e.g. just returning a counter or a pass through function has no design decision).
- How long you roughly spent on the lab, and what would have liked/disliked on the lab.
- If you know some part of the lab is incomplete, write up what parts are not working, how close you think you are, and what part(s) you got stuck on.
