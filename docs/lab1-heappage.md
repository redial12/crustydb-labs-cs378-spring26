_Due Date: Tuesday, February 24th, 2026 at 11:59 pm_

Welcome to CrustyDB! CrustyDB is an academic Rust-based relational database
management system built by [ChiData at The University of
Chicago](https://uchi-db.github.io/chidatasite/), and it is the work of many
contributors. It is designed to be an academic project that can be used for
teaching and a testbed for research projects. 
We will use the CrustyDB platform
to teach you about database system internals.

## The CrustyDB Project
Our approach to building the database system is going to be bottom-up. We start
with the storage manager, the entity responsible for storing the data
on the disk, and then work our way upwards to the query processing engine.

The project is divided into several milestones, each introducing you to a new
concept in database systems. The milestones are as follows:

**Lab 1 - Page (This Milestone)**: You will build a system to persist
data onto fixed-sized pages that store variable values. This milestone requires
you to build out a slotted-page storage system.

**Lab 2 - Heapstore**: In the next milestone, you will continue to
build the storage engine by implementing a heap file storage manager 
called the **heapstore**.

**Lab 3 - Query Operators**: In the third milestone, you will implement
a set of query operators that can be used to execute queries on the data stored in the database.

### Source Code Layout

CrustyDB is set up as a Rust workspace, and various modules/components of the
database are broken into separate packages/crates. To build a specific crate
(for example, the `common` crate), you would use the following command `cargo
build -p common`. Note if a package/crate depends on another crate (e.g. the
`heapstore` crate depends on `common` crate) those crates
will automatically be built as part of the process. 

The crates (located in `src`) are:
- `common` : shared data structures or logical components needed by everything
  in CrustyDB. This includes things like tables, errors, logical query plans,
  ids, some test utilities, etc.
- `storage`: A module that contains multiple storage managers (**SM**'s). The
  storage module is broken into several crates:
  - `storage/heapstore` : a storage manager for storing data in heap files.

:::{note}
For the first lab, you only need to care about the files in
`heapstore` and `common` crates. The other parts of CrustyDB 
are relevant to the follow-up labs onwards.
:::


## Heapstore Design
![pg_hs.png](figures/pg_hs.png)
The heapstore is a storage manager that manages data stored in heap files. Any
value that is to be stored in the database is first converted to a byte array
and then passed into the heapstore to be stored. You'll learn much more about 
heap files and storage managers in the next lab, but in brief:

**Heapfile**:
- A "heapfile" is a `struct` that manages a file. You can think of this as a
wrapper around a file that contains additional metadata 
and methods to help you interact with the file.
- The heapfile struct will contain info to help you utilize that file in the
  context of Crusty, but the file it's linked 
  to is just a regular file in your filesystem.

**Storage Manager**:
- In CrustyDB a storage manager (**SM**) is responsible for persisting all data (aka writing it to disk). 
- A SM in Crusty is agnostic to what is being stored, as it takes a request to
  store a `value` as bytes (a `Vec<u8>`) in a `container`.
- A `container` could represent a table, an index, a stored result, or anything else you want to persist. 
- For example, CrustyDB will create a container for each table/relation stored, and each record will be stored as a `value`.
- Note that there is a 1-1 relationship between containers and heapfiles: you
  can think of 'containers' as wrappers that allow the SM to manage things like heapfile access permissions.
- Once a value is stored, the SM returns a `ValueId` that indicates how it can
  retrieve the value later. The `ValueId` is a struct (defined in the 
  `common::ids` crate), containing all the information needed to locate the 
  value on the correct location in the file. 
- Other components in the CrustyDB system are responsible for
  converting data into bytes for the SM and interpreting bytes from the SM.  
- The SM manages access to all other structs related to storage tasks, such as
  `HeapFile` or `Page`, and acts as the sole interface through which different
  components can persist data or interact with data on disk.

## Slotted Page Architecture

In this lab, you will focus on one piece of functionality of the
`heapstore` crate, the **page**. A page is a fixed-sized data structure that
holds variable-sized values (in our case, records) via slotted storage. 
In slotted storage, each record inserted into a page is associated 
with a slot that points to a contiguous sequence of bytes on the page. 
A record/value will never be split across pages. 
The logic for managing values in a page is as follows:

- When a value is stored in a page, it is associated with a `slot_id` that
  should not change. 
- The page should always assign the lowest available `slot_id` to an insertion. Therefore, if the value associated with a given slot_id is deleted from the page, you should reuse this `slot_id` (see more on deletion below). 
- While the location of the actual bytes of a value in a page *can* change, the slot_id should not. Note that this means that `slot_id`s are not tied to a specific location on the page either. 
- When storing values in a page, the page should insert the value in the 'first' available space in the page. We quote first as it depends on your implementation what first actually means. 
- If a value is deleted, that space should be reused by a later insert.
- When free space is reclaimed and compacted together is up to you; however if
  there is enough free space in the page you should always accept an insertion
  request -- even if the free space was previously used or is not contiguous.
- A page should provide an iterator to return all of the valid values and their corresponding `slot_id` stored in the page.

**Page Size**

A heapfile is made up of a sequence of fixed-sized pages (the size being defined by `PAGE_SIZE` in `common::lib.rs`) concatenated together into one file. 

The bytes that make up a page are broken into:
- The **header**, which holds metadata about the page and the values it stores.
  - Restrictions on the header's composition and size are detailed in the next
  section.
- The **body**, which is where the bytes for values are stored, i.e., the actual records.

Thus the entire page (i.e the header and body) must be packed into a contiguous byte array of size `PAGE_SIZE`.
Note that while values can differ in size, CrustyDB can reject any value that is
larger than `PAGE_SIZE`.

**ValueId**
Every stored value is associated with a `ValueId`. This is defined in
`common::ids`. Each ValueId must specify a `ContainerId` (which is associated with
exactly one container) and then a set of optional Id types. For this lab,
we will use `PageId` and `SlotId` for each `ValueId`. The data types used for
these Ids are also defined in `common::ids`. 

To map to what we have discussed in the class, a `ValueId` is a `RecordId`, the `ContainerId` is the `FileId`, 
which represents a file and a relation.

```
pub type ContainerId = u16;
pub type AtomicContainerId = AtomicU16;
pub type SegmentId = u8;
pub type PageId = u16;
pub type SlotId = u16;
```
The intention is that a `ValueId` should be <= 64 bits. 
This implies that a page cannot have more than `SlotId` slots (`2^16`). 

:::{tip}
If you're confused about containers, `ContainerIds`, etc., you don't have to worry
about their meaning right now; you'll work with them more in the next lab.
This lab will focus on the `PageId` and `SlotId` types.
:::

A related type definition in Page.rs is the `Offset` type, defined as follows:
```rust
pub type Offset = u16;
``` 


The `Offset` type can be used to store a location within the page (as an offset) using just 2 bytes. Note that Rust will default most lengths or indexes to a `usize` which is 8 bytes on most systems (i.e. `u64`). 

While it is usually not safe to downcast a `usize` to a smaller size type
(i.e. `Offset`), if you are careful as to what you are indexing into or
checking the size of, you can downcast (`x as Offset`) - 
assuming your sizes or 
indexes do not exceed the `Offset` size bounds ($2^16$).  When casting or
defining variables, you should use the CrustyDB specific type for the purpose 
and not the original type, as these type definitions can change 
(e.g., always use `SlotId` when referring to a slot number and not `u16`).

**Header Structure**

```
                   8 bytes                6 bytes/slot                          
        ◄──────────────────────────► ◄──────────────────────►                   
        ┌───────────────────────────┬───────────────────────┬──────┐            
      ▲ │       Page Metadata       │   Slot 1 Metadata     │      │ ▲          
      │ ├───────┬───────┐    ┌──────┼───────┬──────┐  ┌─────┤  ... │ │          
      │ │PageId │  ...  │... │  ... │ ...   │ ...  │..│...  │      │ │          
  Page│ ├───────┴───────┴────┴──────┴──────┬┴──────┴──┴─────┴──────┤ │          
Header│ │                                  │   Slot n Metadata     │ │          
      │ │  ...      ...    ...     ...     ├───────┬──────┐  ┌─────┤ │          
      │ │                                  │ ...   │ ...  │..│...  │ │          
      ▼ ├──────────────────────────────────┴───────┴──────┴──┴─────┤ │          
        │                                                          │ │          
        │  ▲                                                       │ │          
        │  │                                                       │ │          
        │  │Free Space                                             │ │          
        │  │                                                       │ │ PAGE_SIZE
        │  │                                                       │ │          
        │  │    Slot Offset                                        │ │          
        │  │    │                                                  │ │          
        │  │    ▼                                                  │ │          
        │  │    ┌─────────────────────┬────────────────────────────┤ │          
        │  │    │Value n              │Value n-1                   │ │          
        │  ▼    │                     │                            │ │          
        ├───────┴─────────────────────┴────────────────────────────┤ │          
        │                                                          │ │          
        │    ... ...          ...            ...        ...        │ │          
        │                                                          │ │          
        ├────────────────────────────────┬─────────────────────────┤ │          
        │Value 1   ...                   │ Value 0                 │ │          
        │                                │                         │ ▼          
        └────────────────────────────────┴─────────────────────────┘            
                                                          
```

The header should be designed carefully. 
We will need enough metadata to manage
the slots efficiently, but we also want to 
minimize the header size to maximize
the space available for storing records. 
We also have to make some assumptions
about the page structure in order to provide useful tests 
that you can use to verify your implementation.

As you decide on your metadata implementation, 
here are some tips to guide you:

- You will need to store the `PageId` of the page in the header. The storage 
  manager must know which page it is reading/writing on.
- You will need to store the number of slots on the page so you know how many
  slots are in use.
- You will need to store the offset of the first free space on the page so you
  know where to insert the next record.
- For each slot, you will need to know the offset of the
  record in the body of the page. As we support variable-length records, 
  you will also need to store the length of each record in the slot metadata.
- You may use up to 8 bytes for Page MetaData and 6 bytes for slot metadata
- Consider how you will handle deleted records. As you complete the Page
  functionality, you will need to reuse the space of deleted records.

## Suggested Steps
We will now provide a rough order of suggested 
steps to complete this lab.
Please note that this is _not_ an exhaustive list of 
all required tests for the lab, 
and you may want to write (and possibly contribute) additional tests.

Note that this lab will have more guidance than later labs, so you
will be completing the required functions for much of this lab. This
lab includes a series of unit and integration tests to test your page's
functionality. This module has a moderate amount of comments. 
Not all packages in CrustyDB will have the same level of comments. 
Working on a moderate-sized
code base with limited comments and documentation is typical when working with
large systems projects, and this should serve as an excellent introduction to
this highly valued skill.

### Read **page.rs** and **heap_page.rs**

The heap page is the basic building block of this lab, so start with this
file/struct (`page.rs` and `heap_page.rs`). Start by reading through the
functions and comments to understand the functionality to be implemented. You
may find it helpful to look at the unit tests in these files to check our
understanding of their expected behavior before you code.

As you read through, think about what data structures/metadata 
you will need to allow for storing variable-sized values. 
You may end up adding new helper/utility functions. 

### Implement the Page struct in **page.rs**

First, in `page.rs`, you should implement the methods for the `Page` struct.
All of the data for a page should be stored in 
the single 4096-byte array within
the struct called `data`. The methods to be implemented for this struct are:

- `new` - Create a new page with the given `PageId`. This function should
initialize the `data` field of the struct and store the `PageID` within `data`.
- `get_page_id` - Get the `PageId` of the page from the `data` field.
- `from_bytes` - Create a new page from a byte array. This function should
  deserialize the byte array into a `Page` struct.
- `to_bytes` - Serialize the page into a byte array. This function should
  serialize the `Page` struct into a byte array.

:::{tip}
Note the inputs of the `from_bytes` and `to_bytes` functions. They take/return
explicit byte arrays that are of size `PAGE_SIZE`. 
The heapstore storage manager
will always provide/expect byte arrays of this size. In this case, do we need 
explicit byte conversions to 
serialize/deserialize the page using these methods?
:::

After you complete these functions, you should be able to run and pass the
`hs_page_create_basic` test. As a reminder, to run a single test, you can use
the following command:

```bash
cargo test -p heapstore hs_page_create_basic
```

You may use the same syntax for running future tests as well.

### Implement the HeapPage trait in **heap_page.rs**

Once your basic page handling code is working, we can move
on to the more advanced functions listed in `heap_page.rs`.

The `HeapPage` trait is a trait that defines the basic functionality of a page
in a heapfile. Your page should continue to implement the slotted page 
architecture described in the background section. 

**Header Utility Functions**

At this point, you should work on your header design. You should add any
helper/utility functions that you think will be useful for managing the header
information. You should think about how you will access and update the header
information as well as information per slot. We don't provide any explicit
designs or tests for this, so you will need to think about what you need to
implement to manage the header information effectively.

Once your header and slot designs are done, a natural starting point is to
implement two utility functions as follows:

- `get_header_size` for getting the current header size when serialized (which
will be useful for figuring out how much free space you really have).
Note that the header size represents all page metadata and slot metadata 
- `get_free_space` to determine the largest block of data free in the page. 


Once these are implemented, you should be able to pass the
`hs_page_sizes_header_free_space` test.


**Inserting and **Retrieving** Values** 

With the header working, move on to `add_value`. This should enable
`hs_page_simple_insert` to pass. This test adds some tuples (as bytes) to the
page and then checks that (1) the slot ids are assigned in order and (2) that
the largest free space and header size are aligned.

After, implement get_value and verify that `hs_page_get_value` passes. At this
point, tests `hs_page_header_size_small`, `hs_page_header_size_full`, and
`hs_page_no_space` should also work.

**Deleting Values**

Next, implement the function `delete_value`, which should free up the bytes
previously used by the slot_id and make the corresponding slot_id available for
the next value that may be inserted. Start with the test
`hs_page_simple_delete`, which only verifies that deleted values are gone. Once
this is working, you will want to ensure you are reusing the space/slots. Please
refer to the sections below which explain space and slotid reclaimation as well
as expected compaction logic for this. I would suggest writing a utility
function that lets you find the first free space on a page and test this
function with `hs_page_get_first_free_space`. Here, you might want to explore
inserting byte vectors of different sizes and see if you can replace/reuse the
space as effectively as possible. You should have `hs_page_delete_insert`
working also at this point.

**Page Iterator**

The last component of this milestone, is writing an iterator to 'walk' through
all valid values stored in a page. 

This is a *consuming* iterator which will move/take ownership of the page. 
You will want to fill in the struct `PageIter`
to hold the metadata for the iterator, the `next` function in the 
`impl Iterator for PageIntoIter`, and `into_iter` in `impl IntoIterator for Page` 
that creates the iterator from a page. With these functions, `hs_page_iter` 
should pass. The tests will assume that values are given in ascending `SlotId` order.  

After completing the iterator, all required functionality in the page should be
complete, and you can run all the tests in the file by running `cargo test -p
heapstore hs_page_` Ensure that you did not break any tests! Congrats!

**Space Reclamation Example**

The page should use deleted space again, but there is no requirement as to when
the page should be reclaimed. In other words, you should never decline an
`add_value` request when there is enough free space on the page, even if it is
scattered in multiple blocks.

To visualize the free space reclamation, imagine the following scenario: We have
a value AA, a value B, a value CC, and three free spaces (-). The `SlotIds` of
AA, B, and CC are 0,1,2 respectively. The physical layout of the page is as
follows:

```
AABCC---
```

After we delete B, the page looks like this:

```
AA-CC---
```

Now, when inserting item D, we could use the free space (`-`) between A & C
(resulting in `AADCC---`) or use free space `-` after `CCC` (resulting
`AA-CCD--`). Let's go with the latter option. Either way, the slotId of D should
be `1` (as we should re-use B's `SlotId`). Now the page looks like this:

```
AA-CCD--
```

Now, if we want to insert `EE`, we only  have one viable spot/space.  The slotId
of `EE` should be `3`, and the page should look like this:

```
AA-CCDEE
```

Inserting `FF` should be rejected (i.e. return None) as it's too large. No
slotId should be assigned.

Inserting `G` must be accepted as there is room. The `slotId` of `G` should be
`4`.


```
AAGCCDEE
```

**Compaction**

If we delete `G` and `EE`, we again have the following three spaces free.
```
AA-CCD--
```

If the page attempts to insert `NNNN`, this request should not work as there is
not enough space to hold `NNNN` (i.e. we should return `None`). Conversely, an
insert for `HHH` should work, as we have three spaces available. 

However, since they are not contiguous spaces the page will need to **compact**
the existing values such that three contigious spaces exist on the page. One of
the possible layouts after compaction is as follows:

```
AACCD---
```

Now, the insertion of `HHH` should result in the following layout:

```
AACCDHHH
```

Note that since slot 3 is the lowest available slot ID, `HHH` should be assigned
a slot ID of 3. Note that when and how you compact data is up to you, but you
must compact values if there is expected free space (accounting for necessary
header data).

## Logging and Debugging

**Logging**

CrustyDB uses the [env_logger](https://docs.rs/env_logger/0.8.2/env_logger/)
crate for logging. Per the docs on the log crate:

The basic use of the log crate is through the five logging macros: `error!`,
`warn!`, `info!`, `debug!` and `trace!` where `error!` represents the
highest-priority log messages and `trace!` represents the lowest. The log messages are
filtered by configuring the log level to exclude messages with a lower priority.
Each of these macros accept format strings similarly to `println!`.

The logging level is set by an environmental variable, `RUST_LOG`. The easiest
way to set the level is when running a cargo command you set the logging level
in the same command, like so: 
```bash
RUST_LOG=debug cargo run --bin server
``` 
However, when running unit tests logging output is suppressed, and the logger is not
initialized. If you want to use logging for a test, you must:
  - Make sure the test in question calls `init()` which is defined in
   `common::testutils` that initializes the logger. It can safely be called
   multiple times.
  - Tell cargo not to capture the output during testing, and set the level to
   `debug` (note the `--` before `--nocapture`): 
   ```
   RUST_LOG=debug cargo test -- --nocapture [opt_test_name]
   ```  

**Debugging**

It is highly recommended that you [set up your IDE to enable Rust
debugging](https://uchi-db.github.io/rust-guide/01_getting_started/03_debugging.html),
as it will allow you to set breakpoints and step through and inspect your code.
In particular, the `rust_analyzer` extension for VSCode is highly recommended
and should provide a hex dump of the page, allowing you to inspect the page
during crucial points in your code.


In addition, we have implemented the `Debug`` `trait for the `Page` struct in
`page.rs`. This will allow you to print out the contents of a page using the
`{:?}` format specifier or by using the `dbg!` macro. For example, after you
have implemented the function `new()` for the `Page` struct, you can print the
hex representation of the page by using `dbg!` in your code:

```rust
let p = Page::new(1);
dbg!(page);
```

The following lines will be printed to the console:
```
[   0] 01 .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  
101 empty lines were hidden
[4080] .  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .
```

Of course, in this simple example, the page is empty except for the first two
bytes, which contains the `PageId`. You can use the `dbg!` macro to print out
more complex pages and verity that the implementation is correct.

## Scoring and Requirements

### Testing
80% of your score on this lab is based on correctness. Correctness is
demonstrated by passing all of the provided unit in the HS package related to
the page. There will be 20 tests in total. To run the provided tests use `cargo test -p heapstore
hs_page` and ensure all the tests pass.

### Quality
10% of your score is based on code quality (following good coding conventions, comments, well-organized functions, etc). We will be looking for the following:

1. **Comments**: You should have comments for all new helper functions, constants and other identifiers that you add.
2. **Proper Types**: You should use suitable custom types. For example, you should use `SlotId` instead of `u16` when referring to a slot number. 
3. **Magic Numbers**: You should avoid magic numbers in your code. If you have a constant that is used in multiple places, you should define it as a constant at the top of the file.
4. **Descriptive Names**: Ensure that variables, functions, and constants have descriptive names that convey their purpose. Please don't use single-letter names or abbreviations unless they are widely recognized and contextually appropriate.

You could use `cargo fmt` to format your code in the right "style" and use 
`cargo clippy` to identify issues about your code, for either performance reasons or code quality. 

### Write Up
10% is based on your write up (`docs/lab1-writeup.txt`). The write up should contain:
 -  A brief description of your solution, in particular what design decisions
    you made and why. This is only needed for the parts of your solution that
    involved some significant work (e.g. just returning a counter or a pass
    through function isn't a design decision).
- How long you roughly spent on the lab, and what you liked/disliked on the lab.
- If you know some part of the lab is incomplete, write up what parts are not working, how close you think you are, and what part(s) you got stuck on.
