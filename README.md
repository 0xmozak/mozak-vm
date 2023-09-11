# Token App (UTXO based)

We are going to build a token based on the UTXO model. We will be using tweaked Solana Architecture as a base for it.

## Architecture

The architecture largely mimics the Computer System. We have a collection of data blobs, with an ownership-relation
between them. Blobs are either owned by another blob, or are master blobs, which own themselves.

Blobs are of two types, Programs and Data. Programs are blobs that can be executed, and Data blobs are blobs that can
be read from and written to. Furthermore, each blob can be either mutable or immutable. Mutable blobs can be written to,
and immutable blobs cannot be written to. Note that this does not mean that immutable blobs cannot be deleted.

Lastly, blobs contain a unique identifier, which is a 256-bit hash of the immutable blob's contents. This means that
if two

### Programs

Programs are blobs that can be executed. They contain a collection of functions, each described by RISC-V instructions.
Each function has a set of arguments, and a return value. Functions can also interact with the other blobs in the
system. They can create new blobs, read from existing blobs, and write to existing blobs, as well as delete blobs.

#### Creating a new blob

To create a new blob, a program sets the type of the blob, the mutability of the blob, and the contents of the blob.

The blobID is calculated as a hash of the programID that owns the blob, the blob's type, and a list of arguments.

The arguments can be either some parts of the data that will be stored in the blob, or the sequential or random number,
or the hash of the previous blob, or anything else. The system is very flexible in this regard.

If the blobID already exists, the system will return an error. This can be caught by the program, and the program can
then decide what to do.

#### Data

Data blobs are all other blobs, that are not executable. For now, we will use JSON as the data format, but we can easily
change it to something more efficient, such as binary.

## Implementation

### Blob Storage

We will use a hashmap to store the blobs. The key will be the blobID, and the value will be the blob itself.
