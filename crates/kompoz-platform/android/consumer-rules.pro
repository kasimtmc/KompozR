# KompozR's classes and their native methods keep their names: the app's library finds them by name (FindClass)
# and registers their natives (RegisterNatives) as it loads.
-keep class dev.kompozr.** {
    native <methods>;
    *;
}
