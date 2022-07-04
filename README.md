# SC2 Build Order Cleaner

Takes a build order formatted by spawningtool.com and cleans it up
a little:

1. Accepts annotations in the form
```
# (Missile Turret) main, natural, 3rd x3
```
   and appends the provided additional description to the build item.
2. Accepts supply counts in the form
```
# [Supply] 7:00 120, 10:00 180
```
   and overrides the supply count at the given timestamps (replays give
   no information about when supply count drops, so this provides a
   way to have more accurate suppry counts).
3. Accepts arbitrary additional things to read out in the form
```
# 5:00 Read this out at 5 minutes
```
4. Merge SCVs produced within 10 seconds.
5. Merge other items produced within 6 seconds.
6. Append the estimated supply count to Supply Depot lines.
7. Append the number of the first 10 things that are built for most
   things.
