BINS = avl_tree_ref diet diet2 diet3
CFLAGS = -Wall -g -fsanitize=address -O3

all: $(BINS)
	./diet3

%: %.c
	gcc $^ -o $@ $(CFLAGS)

clean:
	rm -f $(BINS)

.PHONY: all clean
