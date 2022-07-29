use alloc::vec::Vec;

const MAX_THREADS: usize = 100;
// 含有 m 个元素的一维数组，每个元素代表可利用的某一类资源的数目，
// 其初值是该类资源的全部可用数目，其值随该类资源的分配和回收而动态地改变。
// Available[j] = k，表示第 j 类资源的可用数量为 k。
#[derive(Copy, Clone)]
pub struct Available {
    pub available: [usize; 3], //当前系统只包含三种锁
}
// 分配矩阵 Allocation：n * m 矩阵，表示每类资源已分配给每个线程的资源数。
// Allocation[i,j] = g，则表示线程 i 当前己分得第 j 类资源的数量为 g。
#[derive(Copy, Clone)]
pub struct Allocation {
    pub allocation: [[usize; 3]; MAX_THREADS],
}

// 需求矩阵 Need：n * m 的矩阵，表示每个线程还需要的各类资源数量。
// Need[i,j] = d，则表示线程 i 还需要第 j 类资源的数量为 d
#[derive(Copy, Clone)]
pub struct Need {
    pub need: [[usize; 3]; MAX_THREADS],
}

impl Available {
    pub fn new() -> Available {
        Available {
            available: [0, 0, 0], //三种锁的资源都为0
        }
    }
}

impl Allocation {
    pub fn new() -> Allocation {
        Allocation {
            allocation: [[0, 0, 0]; MAX_THREADS],
        }
    }
}

impl Need {
    pub fn new() -> Need {
        Need {
            need: [[0, 0, 0]; MAX_THREADS],
        }
    }
}
