use core::fmt;
use std::{ops::{Add, AddAssign, Mul}, sync::mpsc, thread};

use anyhow::{Ok, Result};
use tokio::sync::oneshot;

use crate::vector::{dot_product, Vector};


const NUM_THREADS: usize = 4;

pub struct Matrix<T> {
    data: Vec<T>,
    row: usize,
    col: usize,
}

pub struct MsgInput<T> {
    idx: usize,
    row: Vector<T>,
    col: Vector<T>,
}

#[derive(Debug)]
pub struct MsgOutput<T> {
    idx: usize,
    value: T,
}

pub struct Msg<T> {
    input: MsgInput<T>,
    sender: oneshot::Sender<MsgOutput<T>>,
}

pub fn multiply<T>(a: &Matrix<T>, b: &Matrix<T>) -> Result<Matrix<T>>
where T: Copy + Default + Add<Output = T> + Mul<Output = T> + AddAssign + Send + 'static
{
    if a.col != b.row {
        return Err(anyhow::anyhow!("Matrix dimentions mismatch"));
    }
    let senders = (0..NUM_THREADS)
        .map(|_|{
            let (tx, rx)  =mpsc::channel::<Msg<T>>();
            thread::spawn(
                move || {
                    for msg in rx {
                        let value = dot_product(msg.input.row, msg.input.col)?;
                        if let Err(_) = msg.sender.send(MsgOutput { 
                            idx: msg.input.idx, 
                            value }) 
                        {
                            eprintln!("Send error");
                        }
                    }
                   Ok(())
                });
            tx
        }).collect::<Vec<_>>();
    let matrix_len = a.row * b.col;
    let mut data = vec![T::default(); matrix_len];
    let mut receivers = Vec::with_capacity(matrix_len);
    
    for i in 0..a.row {
        for j in 0..b.col {
            let row: Vector<_> = Vector::new(&a.data[i * a.col..(i+1) * a.col]);
            let col_data = b.data[j..].iter().step_by(b.col).copied().collect::<Vec<_>>();
            let col: Vector<T> = Vector::new(col_data);
            let idx = i * b.col + j;
            let input = MsgInput {
                idx,
                row,
                col,
            };
            let (tx, rx) = oneshot::channel();
            let msg = Msg {
                input,
                sender: tx,
            };
            if let Err(e) = senders[idx % NUM_THREADS].send(msg) {
                eprintln!("Error sending message: {}", e);
            }
            receivers.push(rx);
        }
    }
    for mut rx in receivers {
        let output = rx.blocking_recv()?;
        data[output.idx] = output.value;
    }
    Ok(Matrix { data: data, row: a.row, col: b.col })
}

impl <T: fmt::Debug> Matrix<T> 
where T: fmt::Display,
{
    pub fn new(data: impl Into<Vec<T>>, row: usize, col: usize) -> Self {
        Self {
            data: data.into(),
            row,
            col,
        }
    }
}

impl<T> fmt::Display for Matrix<T> 
where T: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{{")?;
        for i in 0..self.row {
            for j in 0..self.col {
                write!(f, "{}", self.data[i * self.col + j])?;
                if j != self.col - 1 {
                    write!(f, " ")?;
                }
            }
            if i != self.row - 1 {
                write!(f, ", ")?;
            }
        }
        write!(f, "}}")?;
        Result::Ok(())
    }
}

impl<T> MsgInput<T> {
    pub fn new(idx: usize, row: Vector<T>, col: Vector<T>) -> Self {
        Self {
            idx,
            row,
            col,
        }
    }
}

impl<T> Msg<T> {
    pub fn new(input: MsgInput<T>, sender: oneshot::Sender<MsgOutput<T>>) -> Self {
        Self {input, sender}
    }
}

impl<T> Mul for Matrix<T> 
where T:  Copy + Default + Add<Output = T> + Mul<Output = T> + AddAssign + Send + 'static{
    type Output = Self;
    fn mul(self, rhs: Self) -> Self::Output {
        multiply(&self, &rhs).expect("Matrix multiply error")
    }
}

#[test]
fn test_matrix_display() -> Result<()> {
    let a = Matrix::new([1, 2, 3, 4], 2, 2);
    let b = Matrix::new([1, 2, 3, 4], 2, 2);
    let c = a * b;
    assert_eq!(c.data, vec![7, 10, 15, 22]);
    assert_eq!(format!("{}", c), "{7 10, 15 22}");
    Ok(())
}

#[test]
fn test_a_can_not_multiply_b() {
    let a = Matrix::new([1, 2, 3, 4, 5, 6], 2, 3);
    let b = Matrix::new([1, 2, 3, 4], 2, 2);
    let c = multiply(&a, &b);
    assert!(c.is_err());
}

#[test]
#[should_panic]
fn test_a_can_not_multiply_b_panic() {
    let a = Matrix::new([1, 2, 3, 4, 5, 6], 2, 3);
    let b = Matrix::new([1, 2, 3, 4], 2, 2);
    let _c = a * b;
}
