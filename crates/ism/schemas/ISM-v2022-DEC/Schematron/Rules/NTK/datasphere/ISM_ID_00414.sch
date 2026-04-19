<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="VALUECHECK"?>
<!-- Original rule id: NTK-ID-00021 -->
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00414">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00414][Error] Datasphere Profile NTK assertions must use ‘datasphere’ as the prefix
        for vocabulary names.</sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        For NTK assertions that use the ‘urn:us:gov:ic:ntk:profile:datasphere’ profile DES,
        ntk:VocabularyType/@ntk:name must start with ‘datasphere:’.</sch:p>
    <sch:rule id="ISM-ID-00414-R1" context="ntk:AccessProfile[ntk:ProfileDes = 'urn:us:gov:ic:ntk:profile:datasphere']/ntk:VocabularyType">
        <sch:assert test="starts-with(@ntk:name, 'datasphere:')" flag="error" role="error">
            [ISM-ID-00414][Error] For ntk:VocabularyType elements in Datasphere NTK assertions, 
            the @ntk:name attribute must start with ‘datasphere:’.</sch:assert>
    </sch:rule>
</sch:pattern>
