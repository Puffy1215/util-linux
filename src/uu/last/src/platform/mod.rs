// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use self::unix::*;

#[cfg(target_os = "openbsd")]
mod openbsd;
#[cfg(target_os = "openbsd")]
pub use self::openbsd::*;
