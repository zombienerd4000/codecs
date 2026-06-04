CC = clang
CFLAGS = -O3 -Wall -Wextra -Wpedantic -std=c99
LDFLAGS = -lm
SRCDIR = src
OBJDIR = obj
TARGET = ctkc0.exe

# windows (msvc linker) doesnt have m.lib; remove -lm on windows
ifdef ComSpec
LDFLAGS =
endif

SRCS = $(SRCDIR)/main.c $(SRCDIR)/bit.c $(SRCDIR)/huff.c $(SRCDIR)/lz.c $(SRCDIR)/codec.c
OBJS = $(SRCS:$(SRCDIR)/%.c=$(OBJDIR)/%.o)

$(TARGET): $(OBJS)
	$(CC) $(CFLAGS) $(LDFLAGS) -o $@ $^

$(OBJDIR)/%.o: $(SRCDIR)/%.c | $(OBJDIR)
	$(CC) $(CFLAGS) -c -o $@ $<

$(OBJDIR):
	mkdir -p $(OBJDIR)

clean:
	rm -rf $(OBJDIR) $(TARGET)

.PHONY: clean
