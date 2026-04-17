KDIR ?= /lib/modules/`uname -r`/build

default:
	$(MAKE) -C $(KDIR) M=$$PWD CFLAGS_MODULE=-g
clean:
	rm -f Module.symvers
	rm -f modules.order
	rm -f *.ko
	rm -f *.mod
	rm -f *.mod.c
	rm -f *.mod.o
	rm -f *.o .*.o
	rm -f *.cmd .*.cmd
	rm -if *.tmp
