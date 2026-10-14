use super::*;
use std::os::unix::fs::symlink;
#[test]
fn pci_usb_discovery_fixture_and_hotplug() {
    // Synthetic sysfs fixture; never a hardware result.
    let root = std::env::temp_dir().join(format!("rfv-sysfs-{}-{}", std::process::id(), now_ns()));
    let pci = root.join("devices/pci/0000:01:00.0");
    let usb = root.join("devices/usb/1-2");
    let driver = root.join("drivers/rtw88_8821ce");
    fs::create_dir_all(&driver).unwrap();
    fs::create_dir_all(&pci).unwrap();
    fs::create_dir_all(usb.join("1-2:1.0")).unwrap();
    fs::write(pci.join("vendor"), "0x10ec\n").unwrap();
    fs::write(pci.join("device"), "0xc821\n").unwrap();
    fs::write(usb.join("idVendor"), "0bda\n").unwrap();
    fs::write(usb.join("idProduct"), "c811\n").unwrap();
    symlink(&driver, pci.join("driver")).unwrap();
    for (name, index, dev) in [("wlan0", "4", pci), ("wlan1", "5", usb.join("1-2:1.0"))] {
        let path = root.join("class/net").join(name);
        fs::create_dir_all(path.join("wireless")).unwrap();
        fs::write(path.join("ifindex"), index).unwrap();
        symlink(dev, path.join("device")).unwrap();
    }
    fs::create_dir_all(root.join("class/net/eth0")).unwrap();
    let found = discover(&root).unwrap();
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].driver, "rtw88_8821ce");
    assert_eq!(found[0].fields["product_id"], "c821");
    assert_eq!(found[1].fields["bus"], "usb");
    assert_eq!(found[1].fields["vendor_id"], "0bda");
    let old = found[0].identity.clone();
    fs::write(root.join("class/net/wlan0/ifindex"), "6").unwrap();
    assert_ne!(discover(&root).unwrap()[0].identity, old);
    fs::remove_dir_all(root.join("class/net/wlan0")).unwrap();
    assert_eq!(discover(&root).unwrap().len(), 1);
    fs::remove_dir_all(root).unwrap();
}
