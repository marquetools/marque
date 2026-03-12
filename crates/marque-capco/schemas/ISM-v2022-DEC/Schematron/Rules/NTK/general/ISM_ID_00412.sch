<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="VALUECHECK"?>
<!-- Original rule id: NTK-ID-00019 -->
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00412">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00412][Error] ntk:VocabularyType must have a source unless being derived from 
        an existing built-in type.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        For each ntk:VocabularyType that does not have a source, make sure that it is one of the built-in types
        or otherwise already declared with a source.
    </sch:p>
    <sch:rule id="ISM-ID-00412-R1" context="ntk:VocabularyType[not(@ntk:source)]">
        <sch:assert test="(some $type in $builtinVocab satisfies $type=@ntk:name)" flag="error" role="error">
            [ISM-ID-00412][Error] ntk:VocabularyType must have a source unless being derived from 
            an existing built-in type.
        </sch:assert>
    </sch:rule>
</sch:pattern>
