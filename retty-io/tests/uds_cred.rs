#[cfg(unix)]
use libc::getegid;
#[cfg(unix)]
use libc::geteuid;
#[cfg(unix)]
use retty_io::net::UnixStream;
#[cfg(unix)]
#[retty_io::test]
async fn test_socket_pair() {
    let (a, b) = UnixStream::pair().unwrap();
    let cred_a = a.peer_cred().unwrap();
    let cred_b = b.peer_cred().unwrap();
    assert_eq!(cred_a, cred_b);

    let uid = unsafe { geteuid() };
    let gid = unsafe { getegid() };

    assert_eq!(cred_a.uid(), uid);
    assert_eq!(cred_a.gid(), gid);
}
