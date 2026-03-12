<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="VALUECHECK"?>
<!-- Original rule id: NTK-ID-00016 -->
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00409">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00409][Error] Grp-ind Profile NTK assertions must use appropriate ‘group’ and ‘individual’ vocabularies
        for vocabulary type definitions.</sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        For NTK assertions that use the ‘urn:us:gov:ic:ntk:profile:grp-ind’ profile DES, 
        ntk:VocabularyType/@name must start with ‘group:’ or ‘individual:’.
    </sch:p>
    <sch:rule id="ISM-ID-00409-R1" context="ntk:AccessProfile[ntk:ProfileDes = 'urn:us:gov:ic:ntk:profile:grp-ind']/ntk:VocabularyType">
        <sch:assert test="starts-with(@ntk:name, 'group:') or starts-with(@ntk:name, 'individual:')" flag="error" role="error">
            [ISM-ID-00409][Error] The @ntk:name attribute must start with either ‘group:’ or ‘individual:’.
        </sch:assert>
    </sch:rule>
</sch:pattern>
