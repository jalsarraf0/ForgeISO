package vm

import "testing"

func TestBuildQemuCommandUEFI(t *testing.T) {
	args := BuildQemuCommand(Mode{UEFI: true}, Params{ISOPath: "/tmp/a.iso", SerialPath: "/tmp/serial.log"})
	found := false
	for i := range args {
		if args[i] == "-bios" {
			found = true
			break
		}
	}
	if !found {
		t.Fatal("expected -bios for UEFI mode")
	}
}
