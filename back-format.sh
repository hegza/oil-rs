# !/bin/sh
SOURCE="events.yaml"

CUR_TAG=$(git describe --abbrev=0)
DIR="./old_data/"
mkdir -p $DIR
CAND_ROOT=$DIR"events-"$CUR_TAG"-"
CAND_EXT=".yaml"
IDX=0
CAND=$CAND_ROOT$IDX$CAND_EXT

# While candidate exists, create next candidate
while [ -f $CAND ]
do
    IDX=$(($IDX+1))
    CAND=$CAND_ROOT$IDX$CAND_EXT
done

# Finally, copy the candidate to be created
echo "cp $SOURCE $CAND"
cp $SOURCE $CAND

