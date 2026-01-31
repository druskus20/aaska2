## Asset loading

Asset are handled in two parts. 

1. Reading: Files are read only once, then cached on disk. Relative paths are
   cannonicalized before reading as to only have one single instance of each
asset.
2. Processing: Salsa tracked queries, to make sure an asset is only processed at most once,
   and only when needed.

Assets are loaded at parsing time. When included either in the top level
configuration or found referenced in a parsed file.

In this way, asset reloading is detected by the cache, with a notify system.

Caveats: when to calculate the hash suffix of an asset? - should the hash be calculated
once generated or from the parsed file. The former ensures that the hash is
ALWAYS unique. The latter gives us the ability to include the asset before fully
generating it. It ideally should be the former, to avoid problems with cache
busting not working when the asset generation code changes.
