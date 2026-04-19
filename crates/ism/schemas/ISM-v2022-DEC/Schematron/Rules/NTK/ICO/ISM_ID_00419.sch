<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="STRUCTURECHECK"?>
<!-- Original rule id: NTK-ID-00026 -->
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00419">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00419][Error] ntk:AccessProfile containing the ntk:AccessPolicy [urn:us:gov:ic:aces:ntk:ico] may not have
        ntk:ProfileDes, ntk:VocabularyType, or ntk:AccessProfileValue elements specified.
        
        Human Readable: When the ICO ACES is referenced, no data content may be specified in the ntk:AccessProfile.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        For every ntk:AccessProfile that has an ntk:AccessPolicy of [urn:us:gov:ic:aces:ntk:ico], 
        the profile should not specify any of the following data elements, ntk:ProfileDes, ntk:VocabularyType, 
        or ntk:AccessProfileValue.
    </sch:p>
    <sch:rule context="ntk:AccessProfile[ntk:AccessPolicy='urn:us:gov:ic:aces:ntk:ico']" id="ISM-ID-00419-R1">
        <sch:assert test="not(ntk:ProfileDes | ntk:VocabularyType | ntk:AccessProfileValue)" flag="error" role="error">
            [ISM-ID-00419][Error] ntk:AccessProfile containing the ntk:AccessPolicy [urn:us:gov:ic:aces:ntk:ico] may not have
            ntk:ProfileDes, ntk:VocabularyType, or ntk:AccessProfileValue elements specified.
            
            Human Readable: When the ICO ACES is referenced, no data content may be specified in the ntk:AccessProfile.
        </sch:assert>
    </sch:rule>
</sch:pattern>
