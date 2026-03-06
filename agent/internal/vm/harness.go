package vm

import "fmt"

type Mode struct {
	BIOS bool
	UEFI bool
}

type Params struct {
	ISOPath    string
	MemoryMB   uint32
	VCPUs      uint32
	SerialPath string
	ShotPath   string
	OVMFPath   string
}

func BuildQemuCommand(mode Mode, p Params) []string {
	memory := p.MemoryMB
	if memory == 0 {
		memory = 2048
	}
	vcpus := p.VCPUs
	if vcpus == 0 {
		vcpus = 2
	}

	args := []string{
		"qemu-system-x86_64",
		"-m", fmt.Sprintf("%d", memory),
		"-smp", fmt.Sprintf("%d", vcpus),
		"-serial", fmt.Sprintf("file:%s", p.SerialPath),
		"-snapshot",
		"-display", "none",
		"-cdrom", p.ISOPath,
		"-no-reboot",
	}

	if p.ShotPath != "" {
		args = append(args, "-monitor", "none", "-vnc", "none")
	}

	if mode.UEFI {
		ovmf := p.OVMFPath
		if ovmf == "" {
			ovmf = "/usr/share/edk2/ovmf/OVMF_CODE.fd"
		}
		args = append(args, "-bios", ovmf)
	}

	if mode.BIOS && !mode.UEFI {
		args = append(args, "-machine", "pc")
	}

	return args
}
