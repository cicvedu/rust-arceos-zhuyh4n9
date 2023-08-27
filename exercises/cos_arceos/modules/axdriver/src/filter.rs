
use cfg_if::cfg_if;

cfg_if! {
    if #[cfg(feature = "net")] {
        use driver_virtio::VirtIoNetDev;
        use driver_common::{BaseDriverOps, DeviceType};
        use driver_net::{NetDriverOps, EthernetAddress, DevResult, NetBufPtr};
        use crate::virtio::VirtIoHalImpl;
    }
}
cfg_if !{
    if #[cfg(bus = "pci")] {
        use driver_pci::{PciRoot, DeviceFunction, DeviceFunctionInfo};
        type VirtIoTransport = driver_virtio::PciTransport;
    } else if #[cfg(bus =  "mmio")] {
        type VirtIoTransport = driver_virtio::MmioTransport;
    }
}

pub struct NetFilter<T> {
    pub inner: T,
}


cfg_if! {
if #[cfg(feature = "net")] {
    impl BaseDriverOps for NetFilter<VirtIoNetDev<VirtIoHalImpl, VirtIoTransport, 64>> {
        fn device_name(&self) -> &str {
            "virtio-net-filter"
        }

        fn device_type(&self) -> DeviceType {
            DeviceType::Net
        }
    }

    #[cfg(feature = "net")]
    impl NetDriverOps for NetFilter<VirtIoNetDev<VirtIoHalImpl, VirtIoTransport, 64>> {
        fn mac_address(&self) -> EthernetAddress {
            self.inner.mac_address()
        }
        /// Whether can transmit packets.
        fn can_transmit(&self) -> bool {
            self.inner.can_transmit()
        }

        /// Whether can receive packets.
        fn can_receive(&self) -> bool {
            self.inner.can_receive()
        }

        /// Size of the receive queue.
        fn rx_queue_size(&self) -> usize {
            self.inner.rx_queue_size()
        }

        /// Size of the transmit queue.
        fn tx_queue_size(&self) -> usize {
            self.inner.tx_queue_size()
        }

        /// Gives back the `rx_buf` to the receive queue for later receiving.
        ///
        /// `rx_buf` should be the same as the one returned by
        /// [`NetDriverOps::receive`].
        fn recycle_rx_buffer(&mut self, rx_buf: NetBufPtr) -> DevResult {
            self.inner.recycle_rx_buffer(rx_buf)
        }

        /// Poll the transmit queue and gives back the buffers for previous transmiting.
        /// returns [`DevResult`].
        fn recycle_tx_buffers(&mut self) -> DevResult {
            self.inner.recycle_tx_buffers()
        }

        /// Transmits a packet in the buffer to the network, without blocking,
        /// returns [`DevResult`].
        fn transmit(&mut self, tx_buf: NetBufPtr) -> DevResult {
            log::warn!("Filter: transmit len[{}]\n", tx_buf.packet_len());
            self.inner.transmit(tx_buf)
        }

        /// Receives a packet from the network and store it in the [`NetBuf`],
        /// returns the buffer.
        ///
        /// Before receiving, the driver should have already populated some buffers
        /// in the receive queue by [`NetDriverOps::recycle_rx_buffer`].
        ///
        /// If currently no incomming packets, returns an error with type
        /// [`DevError::Again`].
        fn receive(&mut self) -> DevResult<NetBufPtr> {
            let rx_buf = self.inner.receive()?;
            log::warn!("Filter: receive len[{}]\n", rx_buf.packet_len());
            return Ok(rx_buf);
        }

        /// Allocate a memory buffer of a specified size for network transmission,
        /// returns [`DevResult`]
        fn alloc_tx_buffer(&mut self, size: usize) -> DevResult<NetBufPtr> {
            self.inner.alloc_tx_buffer(size)
        }
    }
}
}