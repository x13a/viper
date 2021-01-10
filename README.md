# viper

Wipe files with randomized ASCII dicks. It is rust implementation of 
[wipedicks](https://github.com/Drewsif/wipedicks) with some modifications.

## Installation
```sh
$ make
$ make install
```

## Usage
```text
viper [-h|V] [-vv] [-r] [-z] [-n NUM] [-b NUM] FILES

[-h] * Print help and exit
[-V] * Print version and exit
[-v] * Tell what is going on
[-r] * Walk directories recursively
[-z] * First overwrite with zeroes
[-n] * Number of rounds to overwrite (default: 1)
[-b] * Maximum block size in MB (default: 8)
```

## Example

To wipe file:
```sh
$ viper ./delete_me.txt
```

To wipe file with zeroes and then with dicks:
```sh
$ viper -z ./delete_me.txt
```
