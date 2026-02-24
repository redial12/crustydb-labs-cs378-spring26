_Due Date: Tuesday, October 29th, 2024 at 11:59 am (Noon)_

In this lab you will continue building a storage manager that uses
heapfiles to store values/data. As a reminder, in CrustyDB the storage manager
(a.k.a the **SM**) is responsible for persisting all data to disk. An SM in
Crusty is agnostic to what is being stored, as it takes a request to store a
`value` as bytes (a `Vec<u8>`) in a `container`. Once the value is stored, the
SM returns a `ValueId` that indicates how it can retrieve the value later. It is
the responsibility of some other component in the CrustyDB to interpret the
bytes. For example, CrustyDB will create a container for each table/relation
stored, and each record will get stored as a `value`.  The same database could
also store an index as another container, and store each index page as a
`value`.

CrustyDB comes with a 'working' storage manager, the `memstore`, that keeps all
containers in memory using standard data structures. The memstore persists data to
files on shutdown and can re-load the files into memory on start up.  For this
lab you are writing a new SM to replace the memstore with. All the code
you will need to write is in the project/crate `heapstore`. 

This lab includes a series of unit tests and integration tests for testing
functionality. These tests are not exhaustive and you may want to write (and
possibly contribute) additional tests. This module has a moderate number of
comments. 

## Overview

This milestone can be divided into three parts:

1. HeapFile
2. HeapFileIterator
3. StorageManager

We will now provide a brief introduction of each part. More details on each part can be found in the following sections.

**HeapFile**

The `HeapFile` (see `src/storage/heapstore/src/heapfile.rs`) struct is a wrapper on top of a standard filesystem File (i.e. `std::fs::File`). A HeapFile manages a sequence of fixed-sized pages (`PAGE_SIZE` in `common::lib.rs`) stored in the File managed by the heap file object. Sounds familiar? Yes, this is what you 
worked in the previous lab. While your `heap_page` implementation worked
with a `PAGE_SIZE`-sized byte array, we will now be able to read and write data to disk via the `HeapFile` struct. The relationship between `HeapFile` and `HeapPage` is shown in the figure below:

```
            ┌───►        ┌─────────────┐ ◄──┐                          
            │            │             │    │                          
            │            │             │    │                          
            │            │             │    │  
            │            │             │    │                          
            │            │             │    │  4096-byte-sized HeapPage                      
            │            │             │    │                          
            │            │             │    │                          
            │            │             │    │                          
            │            ├─────────────┤ ◄──┘                          
            │            │             │                               
            │            │             │                               
            │            │             │                               
  HeapFile  │            │             │                               
(A sequence │            │             │                               
    of      │            │             │                               
 HeapPages) │            │             │                               
            │            │             │                               
            │            ├─────────────┤                               
            │            │             │                               
            │            │             │                               
            │            │             │                               
            │            │             │                               
            │            │             │                               
            │            │             │                               
            │            │             │                               
            │            │             │                               
            └──►         └─────────────┘                                                             
```

**HeapFileIterator**

In the first lab, you implemented an iterator for the `HeapPage` struct which gives you the ability to iterate over all values stored in a page in ascending order of `SlotId`. Likewise, in this lab, you will implement an iterator for the `HeapFile` struct that allows you to iterate over all values stored in a `HeapFile` in ascending order of `PageId` and `SlotId`. The `HeapFileIterator` struct is defined in `src/storage/heapstore/src/heapfileiter.rs`.

**The Storage Manager Interface**

The storage manager (SM) is the *public* interface to this crate. The SM manages
a set of `containers`, each with its own `ContainerId`, that contain data in the
form of `values` (each `value` is stored as a `Vec<u8>`). Each container SM corresponds 
to a `HeapFile` so that you can find get `ContainerId` from the `HeadFile`. 
In other words, a `container` corresponds to a table in a database, and each `value` corresponds to a record in that table. All read and write requests will act on values in containers and will be handled by the SM. The SM will internally translate those requests into operations against `HeapFile`'s. A SM is required to implement the `StorageTrait` in the `common` crate. 

To summarize, a SM can contain multiple `Container`'s (which is implemented as a `HeapFile`), and each `Container` contains multiple `values` (which are stored as a byte sequence in `HeapPage` as handled by `HeapFile`).

## Suggested Steps
We will now provide a suggested order of steps to complete the lab. Please
note that we are not providing a detailed breakdown of each step as we did in
the previous lab. You should read the entire lab description,
understand the requirements, and cross-reference these instructions with
the provided codebase. Please reach out on Ed or office hours if you feel like 
you have a conceptual gap or are stuck. 

**1. Integrate the Page Lab**

Even if you were unable to complete the page lab in time for the deadline, 
you should take some time to finish the `page.rs` and `heap_page.rs` 
implementations so that you pass all tests, including the stress test.

**2. Work on the HeapFile Interface**

With a working Page, you should move on to completing the `HeapFile` struct and 
implementation in `heapfile.rs`, we have marked the file with `TODO` and `panic`
statements to help you find the places you need to implement.

As noted above, since the `HeapFile` is a wrapper around a `File` object, 
you will use the `std::fs::File` implementation to perform reads and writes on the file. 
In the handout codebase, the file object is defined as `Arc<RwLock<File>>` to 
ensure reading and writing pages to the file in a thread-safe manner (e.g., deal with multiple readers/writers). 
This means the HeapFile uses interior mutability 
such that all functions to HeapFile only pass a reference/borrow to `&self` even though you will need to
modify some state. Please look at the Rust programming book or other materials 
to learn how to use [`Arc<T>`](https://doc.rust-lang.org/rust-by-example/std/arc.html) 
and [`(RwLock<T>)`](https://doc.rust-lang.org/stable/std/sync/struct.RwLock.html#). 

`HeapFile::new` method takes a `PathBuf` as a parameter. This parameter specifies the filename for the underlying file that the HeapFile will use to store pages. Each HeapFile is mapped to a single file, so this filename uniquely identifies a HeapFile.

:::{note} Path vs. PathBuf

A quick note on `Path` vs. `PathBuf`, which you'll be dealing with in this lab in both `StorageManager` and `HeapFile`. We can think of them as analogous to `&str` vs. `String` or `&[]` vs. `Vec`. Path holds a reference to the path string data but doesn't own it (it's a pointer and a length), meaning that it is immutable. Additionally, because it doesn't own the data, `Path` can only reference the data as long as it is available from wherever the data is being stored. `PathBuf` on the other hand actually owns the underlying data and so is mutable and doesn't need to worry about availability concerns. A good rule of thumb is that if you need to store the path, you want a `PathBuf` as you want to own the underlying string data. Otherwise you can take a Path. :::

If you have not worked with File I/O, start with the [simple I/O example from
the Rust book](https://doc.rust-lang.org/book/ch12-02-reading-a-file.html), then
look at the API/documentation for

```rust
use std::fs::{File, OpenOptions};
use std::io::BufWriter;
use std::io::{Seek, SeekFrom};
```

We have also provided an example in the `new` method within `heapfile.rs` for
opening a file with read/write/create permissions. 

Values (e.g., records) are stored in a `HeapFile` in the first available location. If there is no space in the existing pages allocated by the heap file, a new page will be allocated and `HeapFile` will write it in the same underlying filesystem file. Pages are allocated in consecutive order, starting from `PageId` 0.

You will need to figure out how to utilize the fact that Pages are fixed size to ensure that you are able to write pages to a File in the correct order and are subsequently able to read specific Pages from the file. (Hint: Given that a `HeapPage` is 4096-byte sized, you can calculate the offset for a specific page in the file.)

In the comments of the `HeapFile` struct, there is a hint about `HeapFile` not being able to be serialized directly. You can ignore this for now. We will elaborate on this in the "Startup/Shutdown" section.

Once you have implemented the required methods in `heapfile.rs`, you can run 
the following test to verify that you have a working HeapFile implementation:

```bash
cargo test -p heapstore hs_hf_insert
```

Next, you could move on to the `HeapFileIter` or Storage Manager. For both
steps, we are going to not give a suggested order/steps, but suggest that you
look through the code and API and determine the best way to go.

**3. Work on the Heap File Iterator**

The code in `heapfileiter.rs` allows an SM to iterate through all values stored in a heap file. It
will need to walk through all pages and iterate over all values within each
page. We diverge from the standard Rust approach for constructing the iterator
to avoid issues with lifetimes. As a hint, this iterator can internally use the iterator that you already wrote for the HeapPage in the previous lab.

`HeapFileIterator::new` will have a shared ownership of the `HeapFile` and will
create an iterator that will iterate over all values in the `HeapFile` in ascending order of `PageId` and `SlotId` starting from the first valid slot of the first page.

`HeapFileIterator::new_from` will also have a shared ownership of the `HeapFile`. However, it will also take in a `value_id` as a parameter. Recall that a `ValueId` is a struct containing a `PageId` and a `SlotId`. This function will create an iterator that will iterate over all values in the `HeapFile` in ascending order of `PageId` and `SlotId` starting from the `value_id` passed as a parameter.

In both cases, you may ignore `transactionIds` (elaborated later).

You will test your `HeapFileIterator` via the
SM, but feel free to write your own tests here. The tests for the iterator will
assume that PageIds and SlotIds are given in ascending order.

The tests for the heapfile iterator are part of the storage_manager
 (`hs_sm_b_iter_small`). You can pass this test after you have finished the storage manager.

**4. Complete the Storage Manager**
Now, you need to implement the `StorageTrait` trait (as defined in `common::storage_trait`). In the file `storage_manager.rs`, you should 
complete all the required functions as marked with TODO/Panic. Much of SM will be
translating the basic create/read/write/delete requests into the corresponding
actions on HeapPages using the underlying HeapFiles. As a starting point, you may refer to the `memstore` implementation to see how the `StorageTrait` is implemented there, but note that there are some differences: `memstore` is an in-memory implementation, while `heapstore`, as noted earlier, can handle multiple containers (a.k.a. `HeapFiles`).

We have defined a few fields in the `StorageManager` struct to 
keey track of the mapping between `ContainerIds` for the `Containers` that have been
created and the `HeapFiles` which actually manage the data inside a `Container`.

An SM should either be created with a valid directory/path that can persist values or created as a temporary SM used for testing. When an SM shuts down, it should persist enough information to disk so that when it is created again with the same directory/path for persisting data, it is aware of all data that was managed prior to shutdown. In other words, the SM should be able to seamlessly resume if we shut it down and bring it back up. More information on startup/shutdown is provided in the next section.

A few things to note:
 - In `StorageManager::new`, you will create an with `storage_dir` that is passed in as a parameter. Conceptually, this is the directory where the SM will store all data. 
 - In addition to implementing a normal constructor in `StorageManager::new` you
   will need to create a temp SM in `StorageManager::new_test_sm`. The temp SM
   does not need to worry about startup/shutdown serialization and should
   instead create a blank StorageManager with the `is_temp` struct value set to
   `true`.
 - There is a function `reset` which is used for testing. This should clear out
   all data associated with a storage manager and delete all files. This should
   also remove all metadata above the removed data (e.g. remove tables from a
   catalog).
 - SM also uses interior mutability.
 - There are many references to `transactionIds`, permissions, and pins.
   TransactionId and permissions are there for a later (optional) milestone on
   transactions, so you can ignore them for this lab (and is why they are
   _ prefixed). Subsequently, you can ignore the `transaction_finished` and
   `clear_cache` functions for this lab.
 - The function `get_hf_read_write_count` is used for the BufferPool and can be 
   ignored, although it is very simple. If you wish to implement it,
   it simply needs to return a tuple of reads and writes from the underlying
   heap file. If you have a variable called `hf` you could return this via 

    ```rust
    (
    hf.read_count.load(Ordering::Relaxed),
    hf.write_count.load(Ordering::Relaxed),
    )
    ```

 - You may need to add new functions in page/heapfile for some operations.
 - `insert_value` will likely be the trickiest function to implement

:::{note}
**Startup/Shutdown Sequence**

A common lifecycle of a DB is to persist data onto disk, enabling the database
to be stopped while not in use. When DB is restarted, it needs to read from those
files to recover whatever state it was managing before. 
If we shut down Crusty while the SM is managing several
`Containers`/`HeapFiles`, then we want the SM to be aware 
of that data when we bring Crusty back up.
We have implemented part of the `new` and `shutdown` in `StorageTrait` and `StorageManager`. 

You need to implement `StorageManager:new` and `StorageManager::reset`. 
In `StorageManager::reset`, in addition to clearing any data being held by the struct, 
you will need to make sure that any data persisted to disk by previous iterations of Crusty is also
cleared--either `HeapFiles` or saved files previously written by the SM--so that
we're not loading stale state.

## Tests

The tests for the SM are in two locations. 

The first are unit tests in `storage_manager`. You run these with `cargo test -p heapstore hs_sm_`. One of these tests can be slow, so it is ignored by default.
To run this ignored test run `cargo test  -p heapstore hs_sm_  -- --ignored`

The second tests are in `heapstore/tests/` and are integration tests. They are
only allowed to test public functions of the SM, and these tests should pass for
all SM (same tests will exist in the memstore). Run these tests with `cargo test -p heapstore sm_` note this will run the unit tests also as they have `sm_` in the
name.

With this all tests in heapstore should pass: 

```bash
cargo test -p heapstore
```

## Replacing Memstore
For your CrustyDB to use heapstore instead of memstore you will need to change
import statements. The upstream codebase should already have this import
flipped. For example, in `storage/src/lib.rs` you will see the following import statement:

```rust
mod storage_common;

pub use memstore::storage_manager::{StorageManager, STORAGE_DIR};
// pub use heapstore::storage_manager::{StorageManager, STORAGE_DIR};
```

You will need to comment out the memstore line and uncomment the heapstore line.

## Scoring and Requirements

### Testing

**Correctness**:
80% of your score on this milestone is based on correctness. Correctness is
demonstrated by passing all of the provided unit tests in the `heapstore` crate. 
You can run all tests (including the ignored one) using:

```bash
cargo test -p heapstore -- --include-ignored
```

You should see 28 tests in total, with 20 tests from Lab 1.

### Quality
10% of your score is based on code quality (following good coding conventions, comments, well-organized functions, etc). We will be looking for the following:

1. **Comments**: You should have comments for all new helper functions, constants and other identifiers that you add.
2. **Proper Types**: You should use suitable custom types. For example, you should use `SlotId` instead of `u16` when referring to a slot number. 
3. **Magic Numbers**: You should avoid magic numbers in your code. If you have a constant that is used in multiple places, you should define it as a constant at the top of the file.
4. **Descriptive Names**: Ensure that variables, functions, and constants have descriptive names that convey their purpose. Please don't use single-letter names or abbreviations unless they are widely recognized and contextually appropriate.

You could use `cargo fmt` to format your code in the right "style" and use 
`cargo clippy` to identify issues about your code, for either performance reasons or code quality. 
 
### Write Up
10% is based on your write up (`docs/lab2-writeup.txt`). The write up should contain:
 -  A brief description of your solution, in particular what design decisions you took and why. This is only needed for part of your solutions that had some significant work (e.g. just returning a counter or a pass through function has no design decision).
- How long you roughly spent on the lab, and what would have liked/disliked on the lab.
- If you know some part of the lab is incomplete, write up what parts are not working, how close you think you are, and what part(s) you got stuck on.
