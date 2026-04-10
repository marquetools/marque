<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="VALUECHECK"?>
<!-- Original rule id: NTK-ID-00025 -->
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00418">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00418][Error] Sources cannot be overridden. If a built-in vocabulary type is specified and the source 
        attribute is present, it must equal the built-in source.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        When a built-in vocabulary is specified in an ntk:VocabularyType element and the source
        attribute is present, verify that the source specified matches the built-in source value.
    </sch:p>
    <sch:rule id="ISM-ID-00418-R1" context="ntk:VocabularyType[index-of($builtinVocab, @ntk:name) &gt; 0 and @ntk:source]">
        <sch:let name="index" value="index-of($builtinVocab, @ntk:name)"/>
        <sch:assert test="@ntk:source=$builtinVocabSource[$index]" flag="error" role="error">
            [ISM-ID-00418][Error] Sources cannot be overridden. If a built-in vocabulary type is specified 
            and the source attribute is present, it must equal the built-in source. 
            The source [<sch:value-of select="@ntk:source"/>] is invalid with
            respect to the vocabulary type [<sch:value-of select="@ntk:name"/>]. A source of
            [<sch:value-of select="$builtinVocabSource[$index]"/>] is expected.
        </sch:assert>
    </sch:rule>
</sch:pattern>
