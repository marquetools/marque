<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="VALUECHECK"?>
<!-- Original rule id: NTK-ID-00020 -->
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00413">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00413][Error] All vocabularies used must be of a built-in vocabulary type or 
        be defined in this ntk:AccessProfile in an ntk:VocabularyType. 
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        For every ntk:AccessProfileValue element, verify that the value of the vocabulary attribute
        is either one of the built-in vocabulary types or defined in this AccessProfile.
    </sch:p>
    <sch:rule id="ISM-ID-00413-R1" context="ntk:AccessProfileValue">
        <sch:let name="definedTypes" value="preceding-sibling::ntk:VocabularyType/@ntk:name"/>
        <sch:assert test="(some $value in $builtinVocab satisfies $value=@ntk:vocabulary)             or (some $value in $definedTypes satisfies $value=@ntk:vocabulary)" flag="error" role="error">
            [ISM-ID-00413][Error] Undefined vocabulary type: <sch:value-of select="@ntk:vocabulary"/>. 
            All vocabularies used must be of a built-in vocabulary type or 
            be defined in this ntk:AccessProfile in an ntk:VocabularyType. 
        </sch:assert>
    </sch:rule>
</sch:pattern>
