<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="STRUCTURECHECK"?>
<!-- Original rule id: NTK-ID-00023 -->
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00416">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00416][Error] If ntk:AccessProfileValue or ntk:VocabularyType are specified 
        then there must be a Profile DES that defines the use of the ntk:AccessProfile structure.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        When there is content in an ntk:AccessProfile, either ntk:AccessProfileValue or ntk:VocabularyType, 
        then there must also be a ntk:ProfileDes in the AccessProfile.
    </sch:p>
    <sch:rule id="ISM-ID-00416-R1" context="ntk:AccessProfile[ntk:AccessProfileValue or ntk:VocabularyType]">
        <sch:assert test="ntk:ProfileDes" flag="error" role="error">
            [ISM-ID-00416][Error] If ntk:AccessProfileValue or ntk:VocabularyType are specified then there must
            be a Profile DES that defines the use of the ntk:AccessProfile structure.
        </sch:assert>
    </sch:rule>
</sch:pattern>
