# Changelog

---
## [0.0.7-rwmutex](https://github.com/hedon-rust-road/conutils/compare/v0.0.6-condvar..v0.0.7-rwmutex) - 2025-04-25

### ⛰️ Features

- **(remutex)** implement basic read-write mutex - ([674b71a](https://github.com/hedon-rust-road/conutils/commit/674b71ab66aeeb6b4b2300bcd281d21d90d9661d)) - hedon954
- **(rwmutex)** optimize rwmutex to avoid busy-looping writers - ([31944d7](https://github.com/hedon-rust-road/conutils/commit/31944d77a2ce6fbe3bb1e1975afae130a9e0d90a)) - hedon954
- **(rwmutex)** optimize read-write mutex to avoid writer starvation - ([8c0ce29](https://github.com/hedon-rust-road/conutils/commit/8c0ce29bac6d8cc82b3bb2ed49633b68d143cbc3)) - hedon954

<!-- generated by git-cliff -->

---
## [0.0.6-condvar](https://github.com/hedon-rust-road/conutils/compare/v0.0.5-mutex..v0.0.6-condvar) - 2025-04-23

### ⛰️ Features

- **(condvar)** finish basic version of condition varbiable - ([56a6ef1](https://github.com/hedon-rust-road/conutils/commit/56a6ef1d6fc328c2e097164cf4a0b918cbe8aace)) - hedon954
- **(condvar)** optimize condition varbiable to avoid syscall when no waiting threads - ([4e403e9](https://github.com/hedon-rust-road/conutils/commit/4e403e92a22af52dc885f82db0ff4a467e455bfe)) - hedon954
- readme - ([b42990b](https://github.com/hedon-rust-road/conutils/commit/b42990b083d405f6ec781d49eb819fef78468880)) - hedon954

<!-- generated by git-cliff -->

---
## [0.0.5-mutex](https://github.com/hedon-rust-road/conutils/compare/v0.0.3-oneshoe..v0.0.5-mutex) - 2025-04-23

### ⛰️ Features

- **(arc)** implement arc - ([3671487](https://github.com/hedon-rust-road/conutils/commit/36714877ed3b65aae904e4a21a31b34ca2fad834)) - hedon954
- **(arc)** finish implementing arc - ([faad78b](https://github.com/hedon-rust-road/conutils/commit/faad78bf7af678da36cad22d4237b4dec102c4f9)) - hedon954
- **(mutex)** implement basic version of mutex - ([ccc0c87](https://github.com/hedon-rust-road/conutils/commit/ccc0c87225c03260b6a7c5f1de40c02ee15952d2)) - hedon954
- **(mutex)** optimize mutex to only wake_one if there are waiting threads - ([79067cf](https://github.com/hedon-rust-road/conutils/commit/79067cffd3dc248d9f977757a66bb5e31e8c9566)) - hedon954
- **(mutex)** spin before wait - ([b8c963e](https://github.com/hedon-rust-road/conutils/commit/b8c963ec07c2b71cac2b3e481d46bdbde1c92f18)) - hedon954

### 🧪 Tests

- **(arc)** add unit tests - ([ac3b581](https://github.com/hedon-rust-road/conutils/commit/ac3b581d2c4d0d8b0c555f2927f4eb39df5ccf24)) - hedon954

<!-- generated by git-cliff -->

---
## [0.0.3-oneshoe](https://github.com/hedon-rust-road/conutils/compare/v0.0.2-spinlock..v0.0.3-oneshoe) - 2025-04-14

### ⛰️ Features

- **(channel)** v0.0.1 - ([8ed98a9](https://github.com/hedon-rust-road/conutils/commit/8ed98a989349650b380389c05f8cda108c19f9e3)) - hedon954
- **(channel)** implement `one-shot` channel with safe interfaces - ([2a58835](https://github.com/hedon-rust-road/conutils/commit/2a58835a8e43b47d2c548eed316dde549c07045c)) - hedon954
- **(channel)** seperate channel into pairs - ([e637efc](https://github.com/hedon-rust-road/conutils/commit/e637efc8305514e11ff9100d39c9c5994ee10d45)) - hedon954
- **(channel)** use reference for channel to avoid Arc - ([1fd65ee](https://github.com/hedon-rust-road/conutils/commit/1fd65ee395019cd11001e39510fd8f8bee731abe)) - hedon954
- **(channel)** support block for receiver - ([293c3ec](https://github.com/hedon-rust-road/conutils/commit/293c3ecd61830a72df5e9e17e81cf4d24def337c)) - hedon954

<!-- generated by git-cliff -->

---
## [0.0.2-spinlock](https://github.com/hedon-rust-road/conutils/compare/v0.0.1-mpsc..v0.0.2-spinlock) - 2025-04-03

### ⛰️ Features

- **(spinlock)** v0.0.1 - ([52f31da](https://github.com/hedon-rust-road/conutils/commit/52f31da13a3461fa6abce30ecbb3d0ec4be0ac7f)) - hedon954
- **(spinlock)** v0.0.2 - ([ea47a4e](https://github.com/hedon-rust-road/conutils/commit/ea47a4ec49173a13698824c44dbe12f71581589e)) - hedon954
- **(spinlock)** finish implementing spin lock with safe interfaces - ([512ef7d](https://github.com/hedon-rust-road/conutils/commit/512ef7d257c23c7b4cf4c4897e0125c276177cf7)) - hedon954

### 📚 Documentation

- fix path error - ([fbaf0bf](https://github.com/hedon-rust-road/conutils/commit/fbaf0bf7eaf92a733f2ebe23909a244d0cc67e19)) - hedon954

<!-- generated by git-cliff -->

<!-- generated by git-cliff -->
